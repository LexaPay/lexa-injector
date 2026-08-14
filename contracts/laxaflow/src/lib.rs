#![no_std]
use soroban_sdk::{
    contract, contractimpl, contracttype, symbol_short, Env, Address, Symbol, Vec,
};

#[cfg(test)]
mod test;

// ──────────────────────────── Data Keys ────────────────────────────

/// Persistent storage keys. Using typed keys prevents storage collision/overwrite bugs,
/// as Soroban namespaces contract state variables by their rust type mapping.
#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Admin,
    Token,
    Stream(Address),
    Pool(Symbol),
    PoolList,
    Paused,
}

// ──────────────────────────── Data Types ────────────────────────────

/// Per-member salary stream configuration.
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct StreamConfig {
    /// Tokens per second (smallest unit, e.g. stroops).
    pub rate: i128,
    /// Ledger timestamp when the stream started.
    pub start: u64,
    /// Ledger timestamp of the last claim.
    pub last_claim: u64,
    /// Cumulative tokens claimed so far.
    pub total_claimed: i128,
    /// Cliff timestamp (no claims allowed before this time).
    pub cliff: u64,
    /// Status indicating if stream is paused.
    pub paused: bool,
    /// Timestamp of last pause action.
    pub paused_at: u64,
}

/// A revenue-split pool.
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct PoolConfig {
    /// Pool name.
    pub name: Symbol,
    /// Allocation in basis points (1 bps = 0.01%).
    pub bps: u32,
    /// Members who share this pool equally.
    pub members: Vec<Address>,
}

// ──────────────────────────── Contract ────────────────────────────

#[contract]
pub struct LaxaFlow;

#[contractimpl]
impl LaxaFlow {
    // ─── Initialisation ───────────────────────────────────────────

    /// Initialise the payroll contract with an admin and a token.
    pub fn initialize(env: Env, admin: Address, token: Address) {
        if env.storage().persistent().has(&DataKey::Admin) {
            panic!("Already initialized");
        }
        admin.require_auth();
        env.storage().persistent().set(&DataKey::Admin, &admin);
        env.storage().persistent().set(&DataKey::Token, &token);
        env.storage().persistent().set(&DataKey::Paused, &false);
    }

    // ─── Treasury ─────────────────────────────────────────────────

    /// Deposit tokens into the contract treasury.
    pub fn deposit(env: Env, caller: Address, amount: i128) {
        assert!(amount > 0, "Amount must be positive");
        assert!(!Self::is_paused(&env), "Contract is paused");
        caller.require_auth();

        let token = Self::token(&env);
        let client = soroban_sdk::token::Client::new(&env, &token);
        client.transfer(&caller, &env.current_contract_address(), &amount);
    }

    /// Current treasury balance held by this contract.
    pub fn get_balance(env: Env) -> i128 {
        let token = Self::token(&env);
        let client = soroban_sdk::token::Client::new(&env, &token);
        client.balance(&env.current_contract_address())
    }

    // ─── Streaming Payroll ────────────────────────────────────────

    /// Register a team member with a per-second salary rate and optional cliff timestamp.
    pub fn add_member(env: Env, admin: Address, member: Address, rate_per_second: i128, cliff: u64) {
        Self::require_admin(&env, &admin);
        assert!(rate_per_second > 0, "Rate must be positive");

        let now = env.ledger().timestamp();
        let config = StreamConfig {
            rate: rate_per_second,
            start: now,
            last_claim: now,
            total_claimed: 0,
            cliff,
            paused: false,
            paused_at: 0,
        };
        env.storage().persistent().set(&DataKey::Stream(member.clone()), &config);

        // Emit stream addition event
        env.events().publish(
            (symbol_short!("add_str"), member),
            (rate_per_second, cliff),
        );
    }

    /// Deactivate a member's stream (rate → 0). They can still claim
    /// any tokens accrued up to this point.
    pub fn remove_member(env: Env, admin: Address, member: Address) {
        Self::require_admin(&env, &admin);

        let key = DataKey::Stream(member.clone());
        if env.storage().persistent().has(&key) {
            let mut cfg: StreamConfig = env.storage().persistent().get(&key).unwrap();
            let now = env.ledger().timestamp();

            // Settle final accrued balance before zeroing
            let accrued = Self::compute_accrued_internal(&cfg, now);
            if accrued > 0 {
                let token = Self::token(&env);
                let client = soroban_sdk::token::Client::new(&env, &token);
                client.transfer(&env.current_contract_address(), &member, &accrued);
                cfg.total_claimed += accrued;
            }

            cfg.rate = 0;
            cfg.last_claim = now;
            env.storage().persistent().set(&key, &cfg);

            // Emit stream removal event
            env.events().publish(
                (symbol_short!("rem_str"), member),
                accrued,
            );
        }
    }

    /// Claim all accrued streaming salary. Returns the amount transferred.
    pub fn claim(env: Env, member: Address) -> i128 {
        assert!(!Self::is_paused(&env), "Contract is paused");
        member.require_auth();

        let key = DataKey::Stream(member.clone());
        let mut cfg: StreamConfig = env
            .storage()
            .persistent()
            .get(&key)
            .expect("Not a registered member");

        let now = env.ledger().timestamp();
        assert!(now >= cfg.cliff, "Cliff period not met");

        let accrued = Self::compute_accrued_internal(&cfg, now);

        if accrued <= 0 {
            return 0;
        }

        let token = Self::token(&env);
        let client = soroban_sdk::token::Client::new(&env, &token);
        client.transfer(&env.current_contract_address(), &member, &accrued);

        if cfg.paused {
            cfg.paused_at = now;
        }
        cfg.last_claim = now;
        cfg.total_claimed += accrued;
        env.storage().persistent().set(&key, &cfg);

        // Emit claim event
        env.events().publish(
            (symbol_short!("claim"), member),
            accrued,
        );

        accrued
    }

    /// Pause a specific member's salary stream (Admin only).
    pub fn pause_stream(env: Env, admin: Address, member: Address) {
        Self::require_admin(&env, &admin);
        let key = DataKey::Stream(member.clone());
        let mut cfg: StreamConfig = env.storage().persistent().get(&key).expect("Not a registered member");

        if !cfg.paused {
            let now = env.ledger().timestamp();
            // Accrue and freeze current state up to pause time
            let accrued = Self::compute_accrued_internal(&cfg, now);
            cfg.total_claimed += accrued; // Treat accrued up to pause as snapshot
            cfg.last_claim = now;
            cfg.paused = true;
            cfg.paused_at = now;
            env.storage().persistent().set(&key, &cfg);

            // Transfer accrued balance to avoid locking user funds
            if accrued > 0 {
                let token = Self::token(&env);
                let client = soroban_sdk::token::Client::new(&env, &token);
                client.transfer(&env.current_contract_address(), &member, &accrued);
            }

            env.events().publish(
                (symbol_short!("pause_st"), member),
                accrued,
            );
        }
    }

    /// Resume a paused salary stream (Admin only).
    pub fn resume_stream(env: Env, admin: Address, member: Address) {
        Self::require_admin(&env, &admin);
        let key = DataKey::Stream(member.clone());
        let mut cfg: StreamConfig = env.storage().persistent().get(&key).expect("Not a registered member");

        if cfg.paused {
            let now = env.ledger().timestamp();
            cfg.paused = false;
            cfg.last_claim = now;
            cfg.paused_at = 0;
            env.storage().persistent().set(&key, &cfg);

            env.events().publish(
                (symbol_short!("resum_st"), member),
                now,
            );
        }
    }

    /// View accrued but unclaimed earnings for a member.
    pub fn get_accrued(env: Env, member: Address) -> i128 {
        let key = DataKey::Stream(member);
        match env.storage().persistent().get::<_, StreamConfig>(&key) {
            Some(cfg) => {
                let now = env.ledger().timestamp();
                if now < cfg.cliff {
                    0
                } else {
                    Self::compute_accrued_internal(&cfg, now)
                }
            }
            None => 0,
        }
    }

    /// Return a member's full stream configuration.
    pub fn get_member(env: Env, member: Address) -> StreamConfig {
        env.storage()
            .persistent()
            .get(&DataKey::Stream(member))
            .expect("Not a registered member")
    }

    // ─── Revenue-Split Pools ──────────────────────────────────────

    /// Configure the revenue-split matrix.
    /// `pools` must have basis-point allocations that sum to exactly 10 000.
    pub fn set_pools(env: Env, admin: Address, pools: Vec<PoolConfig>) {
        Self::require_admin(&env, &admin);

        let mut total_bps: u32 = 0;
        let mut names: Vec<Symbol> = Vec::new(&env);

        for pool in pools.iter() {
            assert!(pool.bps > 0 && pool.bps <= 10_000, "Invalid basis points");
            total_bps += pool.bps;
            names.push_back(pool.name.clone());
            env.storage()
                .persistent()
                .set(&DataKey::Pool(pool.name.clone()), &pool);
        }

        assert!(total_bps == 10_000, "Basis points must sum to 10000");
        env.storage().persistent().set(&DataKey::PoolList, &names);
    }

    /// One-click treasury distribution: splits `total_amount` across
    /// every configured pool and divides each pool's share equally
    /// among its members.
    pub fn distribute(env: Env, admin: Address, total_amount: i128) {
        Self::require_admin(&env, &admin);
        assert!(total_amount > 0, "Amount must be positive");
        assert!(!Self::is_paused(&env), "Contract is paused");

        let names: Vec<Symbol> = env
            .storage()
            .persistent()
            .get(&DataKey::PoolList)
            .expect("No pools configured");

        let token = Self::token(&env);
        let client = soroban_sdk::token::Client::new(&env, &token);

        for name in names.iter() {
            let pool: PoolConfig = env
                .storage()
                .persistent()
                .get(&DataKey::Pool(name))
                .unwrap();

            let pool_share = (total_amount * pool.bps as i128) / 10_000;
            let n = pool.members.len() as i128;

            if n > 0 && pool_share > 0 {
                let per_member = pool_share / n;
                for m in pool.members.iter() {
                    client.transfer(&env.current_contract_address(), &m, &per_member);
                }
            }
        }

        env.events().publish(
            (symbol_short!("distrib"), admin),
            total_amount,
        );
    }

    /// Returns the address of the token managed by this contract.
    pub fn get_token(env: Env) -> Address {
        Self::token(&env)
    }

    /// Gets global paused status.
    pub fn is_paused(env: &Env) -> bool {
        env.storage().persistent().get(&DataKey::Paused).unwrap_or(false)
    }

    /// Globally pause/unpause contract (Admin only).
    pub fn set_paused(env: Env, admin: Address, paused: bool) {
        Self::require_admin(&env, &admin);
        env.storage().persistent().set(&DataKey::Paused, &paused);
    }

    // ─── Helpers ──────────────────────────────────────────────────

    fn token(env: &Env) -> Address {
        env.storage()
            .persistent()
            .get(&DataKey::Token)
            .expect("Not initialized")
    }

    fn require_admin(env: &Env, caller: &Address) {
        caller.require_auth();
        let admin: Address = env
            .storage()
            .persistent()
            .get(&DataKey::Admin)
            .expect("Not initialized");
        assert!(*caller == admin, "Unauthorized");
    }

    fn compute_accrued_internal(cfg: &StreamConfig, now: u64) -> i128 {
        if cfg.rate == 0 || cfg.paused || now <= cfg.last_claim {
            return 0;
        }
        let elapsed = now - cfg.last_claim;
        (elapsed as i128) * cfg.rate
    }
}
