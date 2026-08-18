#![cfg(test)]
use super::*;
use soroban_sdk::{testutils::Address as _, testutils::Ledger as _, Address, Env, Symbol, Vec};

/// LaxaFlow Payroll Integration and Simulation Tests
///
/// These tests verify contract operations and payroll states:
/// 1. `test_streaming_payroll`: Simulates the progression of ledger time to verify Alice's
///    continuous salary accumulation over 100s and 150s intervals.
/// 2. `test_revenue_split_distribution`: Verifies percentage-based distribution across
///    development and marketing pools using basis points (BPS).
/// 3. `test_unauthorized_add_member`: Assures only the contract admin can manage staff.
/// 4. `test_remove_member_stops_accrual`: Checks that removing a member auto-pays
///    their outstanding balance and stops future salary streams.
/// 5. `test_stream_cliff`: Asserts that claiming remains locked until the cliff period expires.
/// 6. `test_stream_pause_resume`: Verifies that pausing stops stream accrual and resume resumes it.
/// 7. `test_global_emergency_pause`: Assures global pause prevents deposits/distributions.

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

    // Verify token getter endpoint
    assert_eq!(client.get_token(), token_id);

    // Fund the treasury: admin gets minted tokens then deposits
    sac_client.mint(&admin, &100_000);
    client.deposit(&admin, &100_000);
    assert_eq!(client.get_balance(), 100_000);

    // Add a team member earning 10 tokens/second (no cliff)
    let alice = Address::generate(&env);
    client.add_member(&admin, &alice, &10, &0);

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
    client.add_member(&impostor, &member, &10, &0);
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
    client.add_member(&admin, &bob, &100, &0);

    // Advance 10 seconds → 1 000 accrued
    advance_time(&env, 10);

    // Remove Bob — should auto-pay accrued 1 000
    client.remove_member(&admin, &bob);
    assert_eq!(token_client.balance(&bob), 1_000);

    // Advance 100 more seconds — no new accrual
    advance_time(&env, 100);
    assert_eq!(client.get_accrued(&bob), 0);
}

// ─── Test 5: Stream Cliff ──────────────────────────────────────────
#[test]
fn test_stream_cliff() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(LaxaFlow, ());
    let client = LaxaFlowClient::new(&env, &contract_id);

    let token_admin = Address::generate(&env);
    let token_contract = env.register_stellar_asset_contract_v2(token_admin.clone());
    let token_id = token_contract.address();
    let admin = Address::generate(&env);
    client.initialize(&admin, &token_id);

    let charlie = Address::generate(&env);
    let now = env.ledger().timestamp();
    let cliff = now + 100; // Cliff in 100 seconds

    client.add_member(&admin, &charlie, &10, &cliff);

    // Advance 50 seconds (pre-cliff)
    advance_time(&env, 50);
    assert_eq!(client.get_accrued(&charlie), 0); // No visible accrual pre-cliff

    // Claim attempt before cliff should fail
    let res = client.try_claim(&charlie);
    assert!(res.is_err(), "Expected claim to fail before cliff");

    // Advance past cliff (e.g. 60 more seconds → total 110s)
    advance_time(&env, 60);
    assert_eq!(client.get_accrued(&charlie), 1100); // Fully accrues retrospectively
}

// ─── Test 6: Stream Pause and Resume ───────────────────────────────
#[test]
fn test_stream_pause_resume() {
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

    sac_client.mint(&admin, &10_000);
    client.deposit(&admin, &10_000);

    let dave = Address::generate(&env);
    client.add_member(&admin, &dave, &10, &0);

    // Advance 50 seconds → 500 accrued
    advance_time(&env, 50);
    assert_eq!(client.get_accrued(&dave), 500);

    // Admin pauses dave's stream -> dave receives accrued 500 automatically
    client.pause_stream(&admin, &dave);
    assert_eq!(token_client.balance(&dave), 500);

    // Advance 100 seconds while paused
    advance_time(&env, 100);
    assert_eq!(client.get_accrued(&dave), 0); // No new accrual while paused

    // Resume the stream
    client.resume_stream(&admin, &dave);

    // Advance 50 seconds -> Dave should now accrue 500 more
    advance_time(&env, 50);
    assert_eq!(client.get_accrued(&dave), 500);
}

// ─── Test 7: Global Emergency Pause ────────────────────────────────
#[test]
fn test_global_emergency_pause() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(LaxaFlow, ());
    let client = LaxaFlowClient::new(&env, &contract_id);

    let token_admin = Address::generate(&env);
    let token_contract = env.register_stellar_asset_contract_v2(token_admin.clone());
    let token_id = token_contract.address();

    let admin = Address::generate(&env);
    client.initialize(&admin, &token_id);

    assert_eq!(client.is_paused(), false);

    // Trigger emergency pause
    client.set_paused(&admin, &true);
    assert_eq!(client.is_paused(), true);

    // Deposit should fail
    let res = client.try_deposit(&admin, &100i128);
    assert!(res.is_err(), "Deposit should fail when contract is paused");
}

#[test]
#[should_panic(expected = "Not initialized")]
fn test_uninitialized_get_token() {
    let env = Env::default();
    let contract_id = env.register(LaxaFlow, ());
    let client = LaxaFlowClient::new(&env, &contract_id);
    client.get_token();
}

#[test]
fn test_change_admin() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(LaxaFlow, ());
    let client = LaxaFlowClient::new(&env, &contract_id);

    let token_id = Address::generate(&env);
    let admin1 = Address::generate(&env);
    let admin2 = Address::generate(&env);

    client.initialize(&admin1, &token_id);

    // Transfer admin ownership from admin1 to admin2
    client.change_admin(&admin1, &admin2);

    // Verify admin2 can now perform admin functions (like pausing the contract)
    client.set_paused(&admin2, &true);
    assert_eq!(client.is_paused(), true);

    // Verify admin1 is now unauthorized and cannot pause/unpause
    let res = client.try_set_paused(&admin1, &false);
    assert!(res.is_err(), "Previous admin should be unauthorized");
}

#[test]
fn test_update_stream_rate() {
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

    let employee = Address::generate(&env);
    client.add_member(&admin, &employee, &10i128, &0); // 10 tokens/sec

    // Advance 10 seconds -> 100 tokens accrued
    advance_time(&env, 10);
    assert_eq!(client.get_accrued(&employee), 100);

    // Update rate to 20 tokens/sec -> auto-pays old accrued 100 tokens
    client.update_stream_rate(&admin, &employee, &20i128);
    assert_eq!(token_client.balance(&employee), 100);

    // Advance another 10 seconds at new rate -> 200 tokens accrued
    advance_time(&env, 10);
    assert_eq!(client.get_accrued(&employee), 200);
}

#[test]
fn test_check_admin() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(LaxaFlow, ());
    let client = LaxaFlowClient::new(&env, &contract_id);

    let token_id = Address::generate(&env);
    let admin = Address::generate(&env);
    let fake_admin = Address::generate(&env);

    // Initial state (not initialized) should return false
    assert_eq!(client.check_admin(&admin), false);

    client.initialize(&admin, &token_id);

    // After initialization, check_admin should return true for actual admin
    assert_eq!(client.check_admin(&admin), true);
    assert_eq!(client.check_admin(&fake_admin), false);
}



