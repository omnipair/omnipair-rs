use anchor_lang::prelude::*;
use crate::state::factory::{Factory, PairRegistry};

// Define factory-specific errors
#[error_code]
pub enum FactoryError {
    #[msg("Insufficient funds to initialize factory")]
    InsufficientFunds,
    #[msg("Factory already exists for this owner")]
    FactoryAlreadyExists,
    #[msg("Invalid owner")]
    InvalidOwner,
}

// Define an event for factory initialization
#[event]
pub struct FactoryInitializedEvent {
    pub factory: Pubkey,
    pub pair_registry: Pubkey,
    pub owner: Pubkey,
    pub factory_bump: u8,
    pub registry_bump: u8,
}

#[derive(Accounts)]
pub struct InitializeFactory<'info> {
    #[account(
        init,
        payer = payer,
        space = Factory::SIZE,
        seeds = [b"factory", owner.key().as_ref()],
        bump
    )]
    pub factory: Account<'info, Factory>,
    
    #[account(
        init,
        payer = payer,
        space = PairRegistry::SIZE,
        seeds = [b"pair_registry", factory.key().as_ref(), &0u32.to_le_bytes()],
        bump
    )]
    pub pair_registry: Account<'info, PairRegistry>,
    
    /// The owner of the factory
    pub owner: Signer<'info>,
    
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
}

impl<'info> InitializeFactory<'info> {
    pub fn validate(&self) -> Result<()> {
        // Validate that the payer has enough SOL to pay for the transaction
        require!(
            self.payer.lamports() > (Factory::SIZE + PairRegistry::SIZE) as u64 * 1000,
            FactoryError::InsufficientFunds
        );
        
        // Validate that the owner is the same as the payer
        require!(
            self.owner.key() == self.payer.key(),
            FactoryError::InvalidOwner
        );
        
        Ok(())
    }
}

pub fn initialize_factory(ctx: Context<InitializeFactory>) -> Result<()> {
    // Validate the accounts
    ctx.accounts.validate()?;
    
    // Initialize the factory
    let factory = &mut ctx.accounts.factory;
    factory.owner = ctx.accounts.owner.key();
    factory.pair_count = 0;
    factory.pair_registry = ctx.accounts.pair_registry.key();
    
    // Initialize the pair registry
    let pair_registry = &mut ctx.accounts.pair_registry;
    pair_registry.factory = ctx.accounts.factory.key();
    pair_registry.next_registry = None;
    pair_registry.registry_index = 0;
    pair_registry.pairs = Vec::with_capacity(PairRegistry::MAX_PAIRS_PER_REGISTRY);
    
    // Emit event
    emit!(FactoryInitializedEvent {
        factory: ctx.accounts.factory.key(),
        pair_registry: ctx.accounts.pair_registry.key(),
        owner: ctx.accounts.owner.key(),
        factory_bump: ctx.bumps.factory,
        registry_bump: ctx.bumps.pair_registry,
    });
    
    Ok(())
}
