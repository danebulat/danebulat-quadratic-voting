#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::{pallet_prelude::*, traits::fungible};
/// Edit this file to define custom logic or remove it if it is not needed.
/// Learn more about FRAME and the core library of Substrate FRAME pallets:
/// <https://docs.substrate.io/reference/frame-pallets/>
pub use pallet::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

pub type BalanceOf<T> = <<T as Config>::NativeBalance as fungible::Inspect<
	<T as frame_system::Config>::AccountId,
>>::Balance;

use frame_support::sp_runtime::traits::{Convert, Saturating};
use frame_support::traits::fungible::MutateFreeze;
use frame_support::traits::tokens::currency::Currency;
use frame_support::BoundedVec;
use frame_system::pallet_prelude::BlockNumberFor;

#[frame_support::pallet]
pub mod pallet {
	use crate::*;
	use frame_system::pallet_prelude::{OriginFor, *};

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	/// Configure the pallet by specifying the parameters and types on which it depends.
	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// Because this pallet emits events, it depends on the runtime's definition of an event.
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// Type to access the Balances Pallet.
		type NativeBalance: fungible::Inspect<Self::AccountId>
			+ fungible::Mutate<Self::AccountId>
			+ fungible::hold::Inspect<Self::AccountId>
			+ fungible::hold::Mutate<Self::AccountId>
			+ fungible::freeze::Inspect<Self::AccountId>
			+ fungible::freeze::Mutate<Self::AccountId, Id = Self::RuntimeFreezeReason>
			+ Currency<Self::AccountId>;

		/// A helper to convert a block number to a balance type. Might be helpful if you need to do
		/// math across these two types.
		type BlockNumberToBalance: Convert<BlockNumberFor<Self>, BalanceOf<Self>>;

		/// The overarching freeze reason.
		type RuntimeFreezeReason: From<FreezeReason>;

		/// Tiers representing the possible duration of a proposal. A u8 representing how many
		/// weeks the proposal will be active.
		#[pallet::constant]
		type ProposalDurationTier1: Get<u8>;

		#[pallet::constant]
		type ProposalDurationTier2: Get<u8>;

		#[pallet::constant]
		type ProposalDurationTier3: Get<u8>;

		/// The maximum number of proposals an account can vote on.
		#[pallet::constant]
		type MaxProposalsAccountCanVote: Get<u32>;
	}

	/// Voting options to be sent with extrinsic.
	#[derive(Clone, PartialEq, DebugNoBound, TypeInfo, Encode, Decode, MaxEncodedLen)]
	pub enum Vote {
		Aye(u32),
		Nay(u32),
	}

	/// Status of a proposal.
	#[derive(Clone, PartialEq, DebugNoBound, TypeInfo, Encode, Decode, MaxEncodedLen)]
	pub enum Status {
		Active,
		Failed,
		Passed,
	}

	/// Duration tiers for a proposal.
	#[derive(Clone, PartialEq, DebugNoBound, TypeInfo, Encode, Decode, MaxEncodedLen)]
	pub enum Duration {
		Tier1,
		Tier2,
		Tier3,
	}

	/// Proposal data.
	#[derive(Clone, PartialEq, DebugNoBound, TypeInfo, Encode, Decode, MaxEncodedLen)]
	#[scale_info(skip_type_params(T))]
	pub struct Proposal<T: Config> {
		pub status: Status,
		pub deadline: BlockNumberFor<T>,
		pub owner: T::AccountId,
		pub aye_votes: u32,
		pub nay_votes: u32,
	}

	/// Map of registered users.
	#[pallet::storage]
	pub type RegisteredAccounts<T: Config> =
		StorageMap<_, Blake2_128Concat, T::AccountId, bool, ValueQuery>;

	/// Map of proposals, keyed by the proposal hash.
	#[pallet::storage]
	pub type SubmittedProposals<T: Config> = StorageMap<_, Blake2_128Concat, T::Hash, Proposal<T>>;

	/// Double map to store the accounts frozen token amounts for voted on proposals.
	#[pallet::storage]
	pub type AccountVotes<T: Config> = StorageDoubleMap<
		Hasher1 = Blake2_128Concat,
		Key1 = T::AccountId,
		Hasher2 = Twox64Concat,
		Key2 = T::Hash,
		Value = u32,
		QueryKind = OptionQuery,
	>;

	/// Map to track proposals account has voted on. Used for frozen balance calculation.
	#[pallet::storage]
	pub type AccountProposalsMap<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		T::AccountId,
		BoundedVec<T::Hash, T::MaxProposalsAccountCanVote>,
	>;

	/// Events
	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Account has been regiseterd.
		AccountRegistered { who: T::AccountId },
		/// Proposal has been created.
		ProposalCreated { who: T::AccountId, proposal: T::Hash },
		/// Proposal has been closed.
		ProposalClosed { who: T::AccountId, hash: T::Hash },
		/// Account has voted aye.
		VotedAye { who: T::AccountId, proposal: T::Hash, votes: u32 },
		/// Account has voted nay.
		VotedNay { who: T::AccountId, proposal: T::Hash, votes: u32 },
		/// Tokens have been unfrozen.
		TokensClaimed { who: T::AccountId },
	}

	/// A reason for freezing funds.
	#[pallet::composite_enum]
	pub enum FreezeReason {
		#[codec(index = 0)]
		ProposalVote,
	}

	/// Errors to inform users that something went wrong.
	#[pallet::error]
	pub enum Error<T> {
		/// The voting account is not registered to the pallet.
		NotRegistered,
		/// The proposal hash has already been submitted.
		AlreadySubmitted,
		/// Cannot close proposal before deadline.
		ProposalDeadlineNotPassed,
		/// The proposl doesn't exist.
		ProposalNotFound,
		/// Proposal can't be closed if it's still active.
		ProposalStillActive,
		/// Cannot close an inactive proposal.
		ProposalNotActive,
		/// Account doesn't have enough tokens to vote.
		NotEnoughTokens,
		/// No votes for proposal.
		NoVotesFoundForAccount,
		/// Arithemtic error.
		ArithmeticError,
		/// Exceeded amount of proposals account can vote on.
		VoteProposalsExceeded,
		/// Cannot vote on a closed proposal.
		VotingOnClosedProposalNotAllowed,
	}

	/// Calls
	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Register account.
		///
		/// Allows the root origin to register an account to vote on proposals.
		#[pallet::call_index(1)]
		#[pallet::weight(Weight::default())]
		pub fn register_account(origin: OriginFor<T>, who: T::AccountId) -> DispatchResult {
			ensure_root(origin)?;
			RegisteredAccounts::<T>::insert(who.clone(), true);
			Self::deposit_event(Event::AccountRegistered { who });
			Ok(())
		}

		/// Circle of trust register account.
		///
		/// Allows an already-registered account to register another account to vote 
		/// on proposals.
		#[pallet::call_index(2)]
		#[pallet::weight(Weight::default())]
		pub fn circle_of_trust_register_account(
			origin: OriginFor<T>,
			who: T::AccountId,
		) -> DispatchResult {
			let signer = ensure_signed(origin)?;

			if RegisteredAccounts::<T>::contains_key(signer) {
				RegisteredAccounts::<T>::insert(who.clone(), true);
				Self::deposit_event(Event::AccountRegistered { who });
				Ok(())
			} else {
				Err(Error::<T>::NotRegistered.into())
			}
		}

		/// Submit a proposal.
		///
		/// The proposal metadata includes the owner's account ID, the total number of votes 
		/// for the proposal (Ayes and Nays), and the deadline represented by a block number.
		#[pallet::call_index(3)]
		#[pallet::weight(Weight::default())]
		pub fn submit_proposal(
			origin: OriginFor<T>,
			proposal: T::Hash,
			duration: Duration,
		) -> DispatchResult {
			// Check that the function was signed and get the signer.
			let who = ensure_signed(origin)?;

			// Verify that the submitted proposal has not already been stored.
			ensure!(
				!SubmittedProposals::<T>::contains_key(&proposal),
				Error::<T>::AlreadySubmitted
			);

			// Fetch tier from config, representing the duration in weeks.
			let tier = match duration {
				Duration::Tier1 => T::ProposalDurationTier1::get(),
				Duration::Tier2 => T::ProposalDurationTier2::get(),
				Duration::Tier3 => T::ProposalDurationTier3::get(),
			};

			// Get the block number from the FRAME system pallet.
			let current_block = <frame_system::Pallet<T>>::block_number();

			// Calculate the deadline block.
			let week_in_seconds: u32 = 604800;
			let blocks_in_week: u32 =
				week_in_seconds.checked_div(6u32).ok_or(Error::<T>::ArithmeticError)?;
			let duration_in_blocks =
				u32::from(tier).checked_mul(blocks_in_week).ok_or(Error::<T>::ArithmeticError)?;
			let deadline = current_block.saturating_add(duration_in_blocks.into());

			// Instantiate proposal data.
			let metadata: Proposal<T> = Proposal {
				status: Status::Active,
				owner: who.clone(),
				deadline,
				aye_votes: 0,
				nay_votes: 0,
			};

			// Store the proposal data with its hash.
			SubmittedProposals::<T>::insert(&proposal, metadata);

			// Emit an event that the proposal was created.
			Self::deposit_event(Event::ProposalCreated { who, proposal });

			Ok(())
		}

		/// Cast votes on a proposal.
		///
		/// An account is able to vote aye or nay on a proposal by freezing some of
		/// their free balance.
		#[pallet::call_index(4)]
		#[pallet::weight(Weight::default())]
		pub fn cast_vote(origin: OriginFor<T>, proposal: T::Hash, vote: Vote) -> DispatchResult {
			// Check that the function was signed and get the signer.
			let who = ensure_signed(origin)?;

			// Verify vote conditions are met.
			Self::verify_vote_conditions(&who, &proposal)?;

			// Extract amount of votes.
			let votes = Self::extract_votes(&vote);

			// Persist account votes.
			Self::persist_account_vote_tokens(who.clone(), proposal.clone(), votes)?;

			// Update account frozen balance.
			Self::insert_and_update_account_frozen_balance(who.clone(), proposal.clone())?;

			// Update proposal votes.
			Self::add_votes_to_proposal(proposal, &vote)?;

			// Dispatch event confirming vote.
			Self::deposit_event(Self::get_vote_event(who, proposal, &vote, votes));

			Ok(())
		}

		/// Close a proposal if the deadline has passed.
		///
		/// Sets a proposal's status to either Passed or Failed if its deadline has passed.
		/// An error will be returned if the deadline has not yet been reached. After
		/// a proposal has been closed, voters are allowed to unfreeze their voting tokens.
		#[pallet::call_index(5)]
		#[pallet::weight(Weight::default())]
		pub fn close_proposal(origin: OriginFor<T>, hash: T::Hash) -> DispatchResult {
			// Check that the function was signed and get the signer.
			let who = ensure_signed(origin)?;

			// Get proposal from state.
			let mut proposal = Self::get_proposal(hash.clone())?;

			// Check if proposal is already closed.
			ensure!(proposal.status == Status::Active, Error::<T>::ProposalNotActive);

			// Check if deadline is passed and handle appropriately.
			if <frame_system::Pallet<T>>::block_number() >= proposal.deadline {
				// Set status depending on proposal success.
				Self::set_proposal_closed_status(&mut proposal);

				// Update proposal state.
				SubmittedProposals::<T>::set(hash, Some(proposal));

				// Dispatch event.
				Self::deposit_event(Event::ProposalClosed { who, hash });

				Ok(())
			} else {
				Err(Error::<T>::ProposalDeadlineNotPassed.into())
			}
		}

		/// Unfreeze tokens for a voting account.
		///
		/// Tokens can be unfrozen for an account once a proposal's deadline has
		/// passed and its status is not active (Passed or Failed). In other words,
		/// tokens frozen for voting can be returned to an accounts free balance 
		/// after a proposal is closed.
		#[pallet::call_index(6)]
		#[pallet::weight(Weight::default())]
		pub fn claim_back_tokens(origin: OriginFor<T>, proposal_hash: T::Hash) -> DispatchResult {
			// Check that the function was signed and get the signer.
			let who = ensure_signed(origin)?;

			// Get proposal data.
			let proposal = Self::get_proposal(proposal_hash.clone())?;

			// Check that proposal is closed.
			Self::check_proposal_is_closed(&proposal)?;

			// Re-calculate the account's frozen balance based on other proposals currently voted on.
			Self::update_account_frozen_balance_after_claim(who.clone(), proposal_hash.clone())?;

			// Dispatch event.
			Self::deposit_event(Event::TokensClaimed { who });

			Ok(())
		}
	}
}

impl<T: Config> Pallet<T> {
	/// An example of how to get the current block from the FRAME System Pallet.
	pub fn get_current_block_number() -> BlockNumberFor<T> {
		frame_system::Pallet::<T>::block_number()
	}

	/// An example of how to convert a block number to the balance type.
	pub fn convert_block_number_to_balance(block_number: BlockNumberFor<T>) -> BalanceOf<T> {
		T::BlockNumberToBalance::convert(block_number)
	}

	/// Get a proposal from storage.
	pub fn get_proposal(hash: T::Hash) -> Result<Proposal<T>, DispatchError> {
		let mp = SubmittedProposals::<T>::get(hash);

		match mp {
			Some(p) => Ok(p),
			_ => Err(Error::<T>::ProposalNotFound.into()),
		}
	}

	/// Check if proposal has closed.
	pub fn check_proposal_is_closed(proposal: &Proposal<T>) -> DispatchResult {
		if proposal.status == Status::Active {
			Err(Error::<T>::ProposalStillActive.into())
		} else {
			Ok(())
		}
	}

	/// Verify that a vote can go ahead given an account and proposal.
	pub fn verify_vote_conditions(who: &T::AccountId, proposal_hash: &T::Hash) -> DispatchResult {
		let fetched = SubmittedProposals::<T>::get(proposal_hash.clone())
			.ok_or(Error::<T>::ProposalNotFound)?;

		if !RegisteredAccounts::<T>::contains_key(who.clone()) {
			Err(Error::<T>::NotRegistered.into())
		} else if fetched.status != Status::Active {
			Err(Error::<T>::VotingOnClosedProposalNotAllowed.into())
		} else if <frame_system::Pallet<T>>::block_number() >= fetched.deadline {
			Err(Error::<T>::ProposalNotActive.into())
		} else {
			Ok(())
		}
	}

	/// Update a proposal's aye votes in storage.
	pub fn add_votes_to_proposal(hash: T::Hash, vote: &Vote) -> DispatchResult {
		let mut p = Self::get_proposal(hash.clone())?;

		match vote {
			Vote::Aye(n) => {
				let updated = p.aye_votes.saturating_add(*n);
				p.aye_votes = updated;
			},
			Vote::Nay(n) => {
				let updated = p.nay_votes.saturating_add(*n);
				p.nay_votes = updated;
			},
		}

		SubmittedProposals::<T>::set(hash, Some(p));

		Ok(())
	}

	/// Extract vote count.
	pub fn extract_votes(vote: &Vote) -> u32 {
		match vote {
			Vote::Aye(n) => *n,
			Vote::Nay(n) => *n,
		}
	}

	/// Get event from vote.
	pub fn get_vote_event(
		who: T::AccountId,
		proposal: T::Hash,
		vote: &Vote,
		votes: u32,
	) -> Event<T> {
		match vote {
			Vote::Aye(_) => Event::VotedAye { who, proposal, votes },
			Vote::Nay(_) => Event::VotedNay { who, proposal, votes },
		}
	}

	/// Store account's votes for a particular proposal in state.
	pub fn persist_account_vote_tokens(
		who: T::AccountId,
		proposal: T::Hash,
		votes: u32,
	) -> DispatchResult {
		// Verify that the proposal exists.
		ensure!(SubmittedProposals::<T>::contains_key(proposal), Error::<T>::ProposalNotFound);

		// Accumulate votes or persist new vote to state.
		if AccountVotes::<T>::contains_key(who.clone(), proposal) {
			AccountVotes::<T>::try_mutate(who, proposal, |maybe_votes| -> DispatchResult {
				match maybe_votes {
					Some(n) => {
						*maybe_votes = Some(n.saturating_add(votes));
						Ok(())
					},
					_ => Err(Error::<T>::NoVotesFoundForAccount.into()),
				}
			})?;
		} else {
			AccountVotes::<T>::insert(who, proposal, votes);
		}

		Ok(())
	}

	/// Update an account's frozen balance after voting if necessary.
	pub fn insert_and_update_account_frozen_balance(
		who: T::AccountId,
		proposal_hash: T::Hash,
	) -> DispatchResult {
		// Fetch and clone proposal hash vector from state for this account.
		let mut cloned = AccountProposalsMap::<T>::get(who.clone())
			.map_or_else(|| BoundedVec::new(), |v| v.clone());

		// Add proposal hash to the signer's proposals vector.
		if !cloned.contains(&proposal_hash) {
			cloned
				.try_push(proposal_hash.clone())
				.or(Err(Error::<T>::VoteProposalsExceeded))?;
		}

		// Calculate new frozen balance and update account balance.
		let frozen_balance = Self::get_frozen_balance(&who, &cloned)?;

		// Check if signer has enough tokens to vote.
		if T::NativeBalance::free_balance(&who) < frozen_balance.into() {
			return Err(Error::<T>::NotEnoughTokens.into());
		}

		// Update AccountProposalsMap vector for this account.
		AccountProposalsMap::<T>::insert(who.clone(), cloned);

		// Update frozen balance for account.
		T::NativeBalance::extend_freeze(
			&FreezeReason::ProposalVote.into(),
			&who,
			frozen_balance.clone().into(),
		)?;

		Ok(())
	}

	/// Update an account's frozen balance after claiming back tokens.
	pub fn update_account_frozen_balance_after_claim(
		who: T::AccountId,
		proposal_hash: T::Hash,
	) -> DispatchResult {
		// Remove entry from AccountToVoteTokens map.
		if AccountVotes::<T>::contains_key(who.clone(), proposal_hash.clone()) {
			AccountVotes::<T>::remove(who.clone(), proposal_hash.clone());
		}

		// Remove entry from vector in AccountProposalsMap.
		let mut proposals = AccountProposalsMap::<T>::get(who.clone()).unwrap_or(BoundedVec::new());

		let mut i: Option<usize> = None;
		for (j, h) in proposals.as_bounded_slice().iter().enumerate() {
			if *h == proposal_hash {
				i = Some(j);
				break;
			}
		}

		// Update proposals vector for account.
		match i {
			Some(j) => {
				proposals.remove(j);
				AccountProposalsMap::<T>::insert(who.clone(), proposals.clone());
			},
			_ => return Err(Error::<T>::ProposalNotFound.into()),
		}

		// Calculate new frozen balance and update account balance.
		let frozen_balance = Self::get_frozen_balance(&who, &proposals)?;

		if frozen_balance > 0 {
			T::NativeBalance::set_freeze(
				&FreezeReason::ProposalVote.into(),
				&who,
				frozen_balance.into(),
			)?;
		} else {
			T::NativeBalance::thaw(&FreezeReason::ProposalVote.into(), &who)?;
		}

		Ok(())
	}

	/// Return frozen balance for an account.
	pub fn get_frozen_balance(
		who: &T::AccountId,
		proposals: &BoundedVec<T::Hash, T::MaxProposalsAccountCanVote>,
	) -> Result<u32, DispatchError> {
		let votes = proposals
			.iter()
			.map(|h| AccountVotes::<T>::get(who.clone(), h).unwrap_or(0))
			.max()
			.unwrap_or(0);

		let frozen_balance = votes.checked_pow(2).ok_or(Error::<T>::ArithmeticError)?;

		Ok(frozen_balance)
	}

	/// Set proposal closed status.
	pub fn set_proposal_closed_status(proposal: &mut Proposal<T>) {
		if proposal.aye_votes > proposal.nay_votes {
			proposal.status = Status::Passed;
		} else {
			proposal.status = Status::Failed;
		}
	}
}
