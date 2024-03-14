use crate::pallet::{AccountVotes, Duration, Proposal, Status, SubmittedProposals, Vote};
use crate::{mock::*, Error, FreezeReason, RegisteredAccounts};

use frame_support::assert_err;
use frame_support::traits::{fungible::*, Hooks};
use frame_support::traits::tokens::WithdrawConsequence;
use frame_support::{assert_ok, traits::fungible::Mutate};
use sp_core::Hasher;
use sp_runtime::traits::BlakeTwo256;

type NativeBalance = <Test as crate::Config>::NativeBalance;
type THash<T> = <T as frame_system::Config>::Hash;

// Helper function.
fn next_block() {
	let now = System::block_number();
	Voting::on_finalize(now);
	Balances::on_finalize(now);
	System::on_finalize(now);

	System::set_block_number(now + 1);

	System::on_initialize(now + 1);
	Balances::on_initialize(now + 1);
	Voting::on_initialize(now + 1);
}

fn go_to_block(block: u64) {
	let now = System::block_number();
	Voting::on_finalize(now);
	Balances::on_finalize(now);
	System::on_finalize(now);

	System::set_block_number(block);

	System::on_initialize(block);
	Balances::on_initialize(block);
	Voting::on_initialize(block);
}

// Test setup function.
fn submit_proposal_setup() -> THash<Test> {
	// Go past genesis block so events get deposited.
	next_block();

	let alice = 0;
	let bob = 1;
	let charlie = 2;

	// Alice, Bob and Charlie start with 100 native tokens.
	assert_ok!(NativeBalance::mint_into(&alice, 100));
	assert_ok!(NativeBalance::mint_into(&bob, 100));
	assert_ok!(NativeBalance::mint_into(&charlie, 100));

	// Register bob to allow voting.
	assert_ok!(Voting::register_account(RuntimeOrigin::root(), bob));

	next_block();

	// Instantiate proposal and generate its hash.
	let proposal: &str = "My test proposal.";
	let hash = BlakeTwo256::hash(&codec::Encode::encode(&proposal));

	// Submit the proposal.
	assert_ok!(Voting::submit_proposal(
		RuntimeOrigin::signed(alice),
		hash.clone(),
		Duration::Tier1
	));

	next_block();

	hash
}

fn submit_proposal_multiple_setup() -> (THash<Test>, THash<Test>, THash<Test>, THash<Test>) {
	// Go past genesis block so events get deposited.
	System::set_block_number(1);

	let alice = 0;
	let bob = 1;
	let charlie = 2;

	// Alice, Bob and Charlie start with 100 native tokens.
	assert_ok!(NativeBalance::mint_into(&alice, 100));
	assert_ok!(NativeBalance::mint_into(&bob, 100));
	assert_ok!(NativeBalance::mint_into(&charlie, 100));

	// Register bob to allow voting.
	assert_ok!(Voting::register_account(RuntimeOrigin::root(), bob));
	next_block();

	// Instantiate proposal and generate its hash.
	let proposal_1: &str = "My test proposal 1.";
	let proposal_2: &str = "My test proposal 2.";
	let proposal_3: &str = "My test proposal 3.";
	let proposal_4: &str = "My test proposal 4.";

	let hash_1 = BlakeTwo256::hash(&codec::Encode::encode(&proposal_1));
	let hash_2 = BlakeTwo256::hash(&codec::Encode::encode(&proposal_2));
	let hash_3 = BlakeTwo256::hash(&codec::Encode::encode(&proposal_3));
	let hash_4 = BlakeTwo256::hash(&codec::Encode::encode(&proposal_4));

	// Submit the proposal.
	assert_ok!(Voting::submit_proposal(
		RuntimeOrigin::signed(alice),
		hash_1.clone(),
		Duration::Tier1
	));
	assert_ok!(Voting::submit_proposal(
		RuntimeOrigin::signed(alice),
		hash_2.clone(),
		Duration::Tier1
	));
	assert_ok!(Voting::submit_proposal(
		RuntimeOrigin::signed(alice),
		hash_3.clone(),
		Duration::Tier1
	));
	assert_ok!(Voting::submit_proposal(
		RuntimeOrigin::signed(alice),
		hash_4.clone(),
		Duration::Tier1
	));

	next_block();

	(hash_1, hash_2, hash_3, hash_4)
}

#[test]
fn register_account_works() {
	new_test_ext().execute_with(|| {
		// Go past genesis block so events get depositd.
		next_block();

		let bob = 1;

		// Register bob once.
		assert_ok!(Voting::register_account(RuntimeOrigin::root(), bob));

		// Verify state.
		assert!(RegisteredAccounts::<Test>::get(bob));

		// Register bob again and verify state.
		assert_ok!(Voting::register_account(RuntimeOrigin::root(), bob));
		assert!(RegisteredAccounts::<Test>::get(bob));
	});
}

#[test]
fn circle_of_trust_register_account_works() {
	new_test_ext().execute_with(|| {
		next_block();

		let bob = 1;
		let charlie = 2;

		let dave = 3;
		let eve = 4;

		// Alice registers bob.
		assert_ok!(Voting::register_account(RuntimeOrigin::root(), bob));
		// Bob registers charlie.
		assert_ok!(Voting::circle_of_trust_register_account(RuntimeOrigin::signed(bob), charlie));
		// Dave tries to register eve (error).
		assert_err!(
			Voting::circle_of_trust_register_account(RuntimeOrigin::signed(dave), eve),
			Error::<Test>::NotRegistered
		);

		// Assert on-chain state.
		assert!(RegisteredAccounts::<Test>::contains_key(bob));
		assert!(RegisteredAccounts::<Test>::contains_key(charlie));
		assert!(!RegisteredAccounts::<Test>::contains_key(eve));
	});
}

#[test]
fn submit_proposal_works() {
	new_test_ext().execute_with(|| {
		// Go past genesis block so events get depositd.
		next_block();

		let alice = 0;

		// Instantiate proposal and generate its hash.
		let proposal: &str = "My test proposal.";
		let hash = BlakeTwo256::hash(&codec::Encode::encode(&proposal));

		// Dispatch a signed extrinsic.
		assert_ok!(Voting::submit_proposal(
			RuntimeOrigin::signed(alice),
			hash.clone(),
			Duration::Tier1
		));

		// Read pallet storage and assert an expected result.
		let fetched = SubmittedProposals::<Test>::get(hash);

		assert!(fetched.is_some());
		assert_eq!(
			fetched.unwrap(),
			Proposal {
				status: Status::Active,
				owner: alice.into(),
				aye_votes: 0,
				nay_votes: 0,
				deadline: 100_801
			}
		);
	});
}

#[test]
fn not_enough_tokens_error_works() {
	new_test_ext().execute_with(|| {
		next_block();

		let alice = 0;
		let bob = 1;

		// Bob starts with 20 native tokens.
		assert_ok!(NativeBalance::mint_into(&bob, 20));
		// Register bob to allow voting.
		assert_ok!(Voting::register_account(RuntimeOrigin::root(), bob));
		next_block();

		// Instantiate proposal and generate its hash.
		let proposal: &str = "My test proposal.";
		let hash = BlakeTwo256::hash(&codec::Encode::encode(&proposal));

		// Alice submit the proposal.
		assert_ok!(Voting::submit_proposal(
			RuntimeOrigin::signed(alice),
			hash.clone(),
			Duration::Tier1
		));
		next_block();

		// Bob tries to vote on the proposal (error).
		assert_err!(
			Voting::cast_vote(RuntimeOrigin::signed(bob), hash.clone(), Vote::Aye(5)),
			Error::<Test>::NotEnoughTokens
		);
		next_block();

		// Confirm proposal still has no votes.
		assert_eq!(SubmittedProposals::<Test>::get(hash.clone()).unwrap().aye_votes, 0);
		// Bob votes on the proposal successfully.
		assert_ok!(Voting::cast_vote(RuntimeOrigin::signed(bob), hash.clone(), Vote::Aye(3)));
		// Confirm proposal still has votes.
		assert_eq!(SubmittedProposals::<Test>::get(hash.clone()).unwrap().aye_votes, 3);
		// Ensure Bob's tokens are frozen.
		assert_eq!(NativeBalance::balance_frozen(&FreezeReason::ProposalVote.into(), &bob), 9);
	});
}

#[test]
fn vote_aye_works() {
	new_test_ext().execute_with(|| {
		let bob = 1;

		// Alice submits proposal, bob registered to vote with 100 tokens.
		let hash = submit_proposal_setup();

		// Bob votes on the proposal.
		assert_ok!(Voting::cast_vote(RuntimeOrigin::signed(bob), hash.clone(), Vote::Aye(3)));
		next_block();

		// Assert that the account vote state is correct.
		let votes = AccountVotes::<Test>::get(bob, hash.clone()).unwrap();
		assert_eq!(votes, 3);

		// Assert that Bob's balance has been updated correctly.
		assert_eq!(NativeBalance::free_balance(&bob), 100);
		assert_eq!(NativeBalance::total_balance(&bob), 100);

		assert_eq!(NativeBalance::balance_frozen(&FreezeReason::ProposalVote.into(), &bob), 9);
		assert_eq!(NativeBalance::can_withdraw(&bob, 95), WithdrawConsequence::Frozen);
		assert_eq!(NativeBalance::can_withdraw(&bob, 10), WithdrawConsequence::Success);

		// Assert the proposal state is correct.
		let proposal = SubmittedProposals::<Test>::get(hash);
		assert!(proposal.is_some());
		assert_eq!(proposal.clone().unwrap().aye_votes, 3);
		assert_eq!(proposal.clone().unwrap().nay_votes, 0);
	});
}

#[test]
fn vote_nay_works() {
	new_test_ext().execute_with(|| {
		let bob = 1;

		// Alice submits proposal, bob registered to vote with 100 tokens.
		let hash = submit_proposal_setup();

		// Bob votes on the proposal.
		assert_ok!(Voting::cast_vote(RuntimeOrigin::signed(bob), hash.clone(), Vote::Nay(3)));
		next_block();

		// Assert that the account vote state is correct.
		let votes = AccountVotes::<Test>::get(bob, hash.clone()).unwrap();
		assert_eq!(votes, 3);

		// Assert that Bob's balance has been updated correctly.
		assert_eq!(NativeBalance::free_balance(&bob), 100);
		assert_eq!(NativeBalance::total_balance(&bob), 100);

		assert_eq!(NativeBalance::balance_frozen(&FreezeReason::ProposalVote.into(), &bob), 9);
		assert_eq!(NativeBalance::can_withdraw(&bob, 95), WithdrawConsequence::Frozen);
		assert_eq!(NativeBalance::can_withdraw(&bob, 10), WithdrawConsequence::Success);

		// Assert the proposal state is correct.
		let proposal = SubmittedProposals::<Test>::get(hash);
		assert!(proposal.is_some());
		assert_eq!(proposal.clone().unwrap().aye_votes, 0);
		assert_eq!(proposal.clone().unwrap().nay_votes, 3);
	});
}

#[test]
fn account_can_vote_on_same_proposal_multiple_times() {
	new_test_ext().execute_with(|| {
		let bob = 1;

		// Alice submits proposal, bob registered to vote with 100 tokens.
		let hash = submit_proposal_setup();

		// Bob casts 3 votes on the proposal.
		assert_ok!(Voting::cast_vote(RuntimeOrigin::signed(bob), hash.clone(), Vote::Aye(3)));
		next_block();
		// Bob casts 5 more votes on the proposal.
		assert_ok!(Voting::cast_vote(RuntimeOrigin::signed(bob), hash.clone(), Vote::Aye(5)));
		next_block();

		// Assert that the account vote state is correct.
		assert_eq!(AccountVotes::<Test>::get(bob, hash.clone()).unwrap(), 8);

		// Assert that Bob's balance has been updated correctly.
		assert_eq!(NativeBalance::free_balance(&bob), 100);
		assert_eq!(NativeBalance::total_balance(&bob), 100);

		// Frozen balance should be 64 tokens.
		assert_eq!(NativeBalance::balance_frozen(&FreezeReason::ProposalVote.into(), &bob), 64);
		assert_eq!(NativeBalance::can_withdraw(&bob, 95), WithdrawConsequence::Frozen);
		assert_eq!(NativeBalance::can_withdraw(&bob, 10), WithdrawConsequence::Success);

		// Assert the proposal state is correct.
		let proposal = SubmittedProposals::<Test>::get(hash);
		assert!(proposal.is_some());
		assert_eq!(proposal.clone().unwrap().aye_votes, 8);
		assert_eq!(proposal.clone().unwrap().nay_votes, 0);
	});
}

#[test]
fn unregistered_account_cannot_vote() {
	new_test_ext().execute_with(|| {
		let charlie = 2;

		// Alice submits proposal, bob registered to vote with 100 tokens.
		let hash = submit_proposal_setup();

		assert_err!(
			Voting::cast_vote(RuntimeOrigin::signed(charlie), hash, Vote::Aye(1)),
			Error::<Test>::NotRegistered
		);
	});
}

#[test]
fn cannot_vote_on_closed_proposal() {
	new_test_ext().execute_with(|| {
		let alice = 0;
		let bob = 1;

		// Alice submits proposal, bob registered to vote with 100 tokens.
		let hash = submit_proposal_setup();

		// Bob votes on the proposal.
		assert_ok!(Voting::cast_vote(RuntimeOrigin::signed(bob), hash.clone(), Vote::Aye(3)));
		// Go past deadline.
		go_to_block(100_900);
		// Close proposal.
		assert_ok!(Voting::close_proposal(RuntimeOrigin::signed(alice), hash.clone()));
		// Go to next block.
		go_to_block(100_901);

		// Bob tries to vote again (error).
		assert_err!(
			Voting::cast_vote(RuntimeOrigin::signed(bob), hash.clone(), Vote::Aye(3)),
			Error::<Test>::VotingOnClosedProposalNotAllowed
		)
	});
}

#[test]
fn cannot_vote_on_too_many_proposals() {
	new_test_ext().execute_with(|| {
		let bob = 1;

		// Alice submits proposal, bob registered to vote with 100 tokens.
		let (hash_1, hash_2, hash_3, hash_4) = submit_proposal_multiple_setup();

		// Bob votes on three proposals successfully.
		assert_ok!(Voting::cast_vote(RuntimeOrigin::signed(bob), hash_1.clone(), Vote::Aye(2)));
		assert_ok!(Voting::cast_vote(RuntimeOrigin::signed(bob), hash_2.clone(), Vote::Aye(2)));
		assert_ok!(Voting::cast_vote(RuntimeOrigin::signed(bob), hash_3.clone(), Vote::Aye(2)));

		// Bob tries to vote on a fourth proposal (error).
		assert_err!(
			Voting::cast_vote(RuntimeOrigin::signed(bob), hash_4.clone(), Vote::Aye(2)),
			Error::<Test>::VoteProposalsExceeded
		);
	});
}

#[test]
fn close_proposal_works() {
	new_test_ext().execute_with(|| {
		let alice = 0;
		let bob = 1;

		// Alice submits proposal, bob registered to vote with 100 tokens.
		let hash = submit_proposal_setup();

		// Bob votes on the proposal (first time).
		assert_ok!(Voting::cast_vote(RuntimeOrigin::signed(bob), hash.clone(), Vote::Aye(3)));
		// Go past deadline.
		go_to_block(100_900);
		// Close proposal.
		assert_ok!(Voting::close_proposal(RuntimeOrigin::signed(alice), hash.clone()));
		// Go past deadline block.
		go_to_block(100_901);

		// Assert status of proposal is correct.
		let proposal: Option<Proposal<Test>> = SubmittedProposals::<Test>::get(hash.clone());
		assert!(proposal.is_some());

		let p = proposal.unwrap();
		assert_eq!(p.status, Status::Passed);
		assert_eq!(p.aye_votes, 3);
	});
}

#[test]
fn anyone_can_close_proposal() {
	new_test_ext().execute_with(|| {
		let alice = 0;
		let bob = 1;

		// Alice submits proposal, bob registered to vote with 100 tokens.
		let hash = submit_proposal_setup();
		// Go past the deadline.
		go_to_block(100_901);

		// Bob closes the proposal.
		assert_ok!(Voting::close_proposal(RuntimeOrigin::signed(bob), hash.clone()));
		// Alice tries to close the proposal. (already closed)
		assert_err!(
			Voting::close_proposal(RuntimeOrigin::signed(alice), hash.clone()),
			Error::<Test>::ProposalNotActive
		);
	});
}

#[test]
fn frozen_balance_is_correct_after_multiple_votes() {
	new_test_ext().execute_with(|| {
		let alice = 0;
		let bob = 1;

		// Alice submits proposal, bob registered to vote with 100 tokens.
		let (hash_1, hash_2, hash_3, _hash_4) = submit_proposal_multiple_setup();

		// Bob votes 9 tokens on proposal 1.
		assert_ok!(Voting::cast_vote(RuntimeOrigin::signed(bob), hash_1.clone(), Vote::Aye(3)));
		// Bob votes 16 tokens on proposal 2.
		assert_ok!(Voting::cast_vote(RuntimeOrigin::signed(bob), hash_2.clone(), Vote::Aye(4)));
		// Assert frozen balance is max(9, 16).
		assert_eq!(NativeBalance::balance_frozen(&FreezeReason::ProposalVote.into(), &bob), 16);

		// Bob votes 25 tokens on proposal 3.
		assert_ok!(Voting::cast_vote(RuntimeOrigin::signed(bob), hash_3.clone(), Vote::Aye(5)));
		// Assert frozen balance is max(9, 16, 25).
		assert_eq!(NativeBalance::balance_frozen(&FreezeReason::ProposalVote.into(), &bob), 25);

		// Bob votes another 2 tokens on proposal, now requiring 64 votes (8th vote).
		assert_ok!(Voting::cast_vote(RuntimeOrigin::signed(bob), hash_3.clone(), Vote::Aye(3)));

		// Go past deadline block.
		System::set_block_number(100_910);
		// Assert frozen balance is max(9, 16, 49).
		assert_eq!(NativeBalance::balance_frozen(&FreezeReason::ProposalVote.into(), &bob), 64);

		// Alice closes proposal 3.
		assert_ok!(Voting::close_proposal(RuntimeOrigin::signed(alice), hash_3.clone()));
		next_block();

		// Bob claims back his tokens on proposal 3 (currently 29).
		assert_ok!(Voting::claim_back_tokens(RuntimeOrigin::signed(bob), hash_3.clone()));
		next_block();

		// Assert bob's new frozen balance is now 16.
		assert_eq!(NativeBalance::balance_frozen(&FreezeReason::ProposalVote.into(), &bob), 16);

		// Alice closes proposal 1 and 2.
		assert_ok!(Voting::close_proposal(RuntimeOrigin::signed(alice), hash_2.clone()));
		assert_ok!(Voting::close_proposal(RuntimeOrigin::signed(alice), hash_1.clone()));
		next_block();

		// Bob claims back his 16 tokens on proposal 2, leaving a frozen balance of 9.
		assert_ok!(Voting::claim_back_tokens(RuntimeOrigin::signed(bob), hash_2.clone()));
		assert_eq!(NativeBalance::balance_frozen(&FreezeReason::ProposalVote.into(), &bob), 9);

		// Bob claims back his 9 tokens on proposal 1, leaving a frozen balance of 0.
		assert_ok!(Voting::claim_back_tokens(RuntimeOrigin::signed(bob), hash_1.clone()));
		assert_eq!(NativeBalance::balance_frozen(&FreezeReason::ProposalVote.into(), &bob), 0);

		assert_eq!(NativeBalance::free_balance(&bob), 100);
		assert_eq!(NativeBalance::total_balance(&bob), 100);
	});
}

#[test]
fn claim_back_tokens_works() {
	new_test_ext().execute_with(|| {
		let alice = 0;
		let bob = 1;

		// Alice submits proposal, bob registered to vote with 100 tokens.
		let hash = submit_proposal_setup();

		// Bob votes on the proposal (first time).
		assert_ok!(Voting::cast_vote(RuntimeOrigin::signed(bob), hash.clone(), Vote::Aye(3)));
		// Go past deadline.
		System::set_block_number(100_900);
		// Close proposal.
		assert_ok!(Voting::close_proposal(RuntimeOrigin::signed(alice), hash.clone()));
		// Go to next block.
		next_block();
		// Bob claims back his tokens.
		assert_ok!(Voting::claim_back_tokens(RuntimeOrigin::signed(bob), hash.clone()));
		next_block();

		// Assert that Bob now has no forzen tokens.
		assert_eq!(NativeBalance::free_balance(&bob), 100);
		assert_eq!(NativeBalance::total_balance(&bob), 100);
		assert_eq!(NativeBalance::balance_frozen(&FreezeReason::ProposalVote.into(), &bob), 0);
		assert_eq!(NativeBalance::can_withdraw(&bob, 98), WithdrawConsequence::Success);
	});
}
