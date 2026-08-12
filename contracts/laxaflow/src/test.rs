#![cfg(test)]
use super::*;
use soroban_sdk::{testutils::Address as _, testutils::Ledger as _, Address, Env, Symbol, Vec};

// ─── Helper: advance the ledger timestamp ──────────────────────────
fn advance_time(env: &Env, seconds: u64) {
    let current = env.ledger().timestamp();
    env.ledger().with_mut(|li| {
        li.timestamp = current + seconds;
    });
}

// ─── Test 1: Streaming payroll — deposit, add member, claim ────────
#[test]
fn test_streaming_payroll() {
    let env = Env::default();
    env.mock_all_auths();

    // Deploy contract
    let contract_id = env.register(LaxaFlow, ());
    let client = LaxaFlowClient::new(&env, &contract_id);

    // Deploy a mock SAC token
    let token_admin = Address::generate(&env);
    let token_contract = env.register_stellar_asset_contract_v2(token_admin.clone());
    let token_id = token_contract.address();
    let token_client = soroban_sdk::token::Client::new(&env, &token_id);
    let sac_client = soroban_sdk::token::StellarAssetClient::new(&env, &token_id);

    // Initialize the payroll contract
    let admin = Address::generate(&env);
    client.initialize(&admin, &token_id);

    // Fund the treasury: admin gets minted tokens then deposits
    sac_client.mint(&admin, &100_000);
    client.deposit(&admin, &100_000);
    assert_eq!(client.get_balance(), 100_000);

    // Add a team member earning 10 tokens/second
    let alice = Address::generate(&env);
    client.add_member(&admin, &alice, &10);

    // Advance 100 seconds → Alice should have accrued 1 000
    advance_time(&env, 100);
    assert_eq!(client.get_accrued(&alice), 1_000);

    // Alice claims
    let claimed = client.claim(&alice);
    assert_eq!(claimed, 1_000);
    assert_eq!(token_client.balance(&alice), 1_000);
    assert_eq!(client.get_balance(), 99_000);

    // Advance another 50 seconds → 500 more accrued
    advance_time(&env, 50);
    assert_eq!(client.get_accrued(&alice), 500);
}

// ─── Test 2: Revenue-split distribution ────────────────────────────
#[test]
fn test_revenue_split_distribution() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(LaxaFlow, ());
    let client = LaxaFlowClient::new(&env, &contract_id);

    // Token setup
    let token_admin = Address::generate(&env);
    let token_contract = env.register_stellar_asset_contract_v2(token_admin.clone());
    let token_id = token_contract.address();
    let token_client = soroban_sdk::token::Client::new(&env, &token_id);
    let sac_client = soroban_sdk::token::StellarAssetClient::new(&env, &token_id);

    let admin = Address::generate(&env);
    client.initialize(&admin, &token_id);

    // Fund treasury with 10 000 tokens
    sac_client.mint(&admin, &10_000);
    client.deposit(&admin, &10_000);

    // Create team members
    let dev1 = Address::generate(&env);
    let dev2 = Address::generate(&env);
    let mkt1 = Address::generate(&env);

    // Configure pools: 60% dev, 40% marketing
    let dev_pool = PoolConfig {
        name: Symbol::new(&env, "dev"),
        bps: 6_000,
        members: Vec::from_array(&env, [dev1.clone(), dev2.clone()]),
    };
    let mkt_pool = PoolConfig {
        name: Symbol::new(&env, "marketing"),
        bps: 4_000,
        members: Vec::from_array(&env, [mkt1.clone()]),
    };
    let pools = Vec::from_array(&env, [dev_pool, mkt_pool]);
    client.set_pools(&admin, &pools);

    // Distribute 10 000 tokens
    client.distribute(&admin, &10_000);

    // Dev pool: 60% of 10 000 = 6 000, split 2 ways → 3 000 each
    assert_eq!(token_client.balance(&dev1), 3_000);
    assert_eq!(token_client.balance(&dev2), 3_000);

    // Marketing pool: 40% of 10 000 = 4 000, 1 member → 4 000
    assert_eq!(token_client.balance(&mkt1), 4_000);

    // Treasury should be empty
    assert_eq!(client.get_balance(), 0);
}

// ─── Test 3: Unauthorized access should panic ──────────────────────
#[test]
#[should_panic(expected = "Unauthorized")]
fn test_unauthorized_add_member() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(LaxaFlow, ());
    let client = LaxaFlowClient::new(&env, &contract_id);

    let token_admin = Address::generate(&env);
    let token_contract = env.register_stellar_asset_contract_v2(token_admin.clone());
    let token_id = token_contract.address();

    let admin = Address::generate(&env);
    client.initialize(&admin, &token_id);

    // An impostor tries to add a member
    let impostor = Address::generate(&env);
    let member = Address::generate(&env);
    client.add_member(&impostor, &member, &10);
}

// ─── Test 4: Remove member stops accrual ───────────────────────────
#[test]
fn test_remove_member_stops_accrual() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(LaxaFlow, ());
    let client = LaxaFlowClient::new(&env, &contract_id);

    let token_admin = Address::generate(&env);
    let token_contract = env.register_stellar_asset_contract_v2(token_admin.clone());
    let token_id = token_contract.address();
    let token_client = soroban_sdk::token::Client::new(&env, &token_id);
    let sac_client = soroban_sdk::token::StellarAssetClient::new(&env, &token_id);

    let admin = Address::generate(&env);
    client.initialize(&admin, &token_id);

    sac_client.mint(&admin, &50_000);
    client.deposit(&admin, &50_000);

    let bob = Address::generate(&env);
    client.add_member(&admin, &bob, &100);

    // Advance 10 seconds → 1 000 accrued
    advance_time(&env, 10);

    // Remove Bob — should auto-pay accrued 1 000
    client.remove_member(&admin, &bob);
    assert_eq!(token_client.balance(&bob), 1_000);

    // Advance 100 more seconds — no new accrual
    advance_time(&env, 100);
    assert_eq!(client.get_accrued(&bob), 0);
}
