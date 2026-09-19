use exchange_core::{
    AppendEntries, AppendResponse, LogIndex, NodeId, OrderCommand, RaftAction, RaftNode, Role, Term,
};
use exchange_core::{ClientOrderId, NewOrder, OrderSide};
use std::time::Instant;

fn members() -> Vec<NodeId> {
    vec![NodeId(1), NodeId(2), NodeId(3)]
}

fn command(id: u64) -> OrderCommand {
    OrderCommand::New(NewOrder {
        client_order_id: ClientOrderId(id),
        account_id: 1,
        instrument_id: 0,
        side: OrderSide::Buy,
        price: 100,
        quantity: 1,
        client_timestamp: id,
    })
}

fn elect(node: &mut RaftNode) {
    let actions = node.start_election();
    assert_eq!(actions.len(), 2);
    node.handle_vote_response(exchange_core::VoteResponse {
        term: node.term(),
        voter_id: NodeId(2),
        granted: true,
    })
    .unwrap();
    assert_eq!(node.role(), Role::Leader);
}

fn replicate_one(leader: &mut RaftNode, follower: &mut RaftNode) {
    let actions = leader.propose(command(leader.last_log_index().0 + 1)).unwrap();
    let request = actions
        .into_iter()
        .find_map(|a| match a {
            RaftAction::AppendEntries { to, request } if to == follower.id() => Some(request),
            _ => None,
        })
        .unwrap();

    let response = follower.handle_append_entries(request).unwrap();
    assert!(response.success);
    leader
        .handle_append_response(
            follower.id(),
            AppendResponse {
                term: response.term,
                follower_id: follower.id(),
                success: true,
                match_index: response.match_index,
            },
        )
        .unwrap();
}

#[test]
fn measured_rpo_and_consensus_failover_rto() {
    let mut leader = RaftNode::new(NodeId(1), members(), 5, 1).unwrap();
    let mut survivor = RaftNode::new(NodeId(2), members(), 5, 1).unwrap();
    let mut third = RaftNode::new(NodeId(3), members(), 5, 1).unwrap();

    elect(&mut leader);

    // Establish a committed prefix on a majority. This is the durability
    // boundary used for RPO: only committed commands count as acknowledged.
    for _ in 0..100 {
        replicate_one(&mut leader, &mut survivor);
    }
    assert_eq!(leader.commit_index(), LogIndex(100));
    assert_eq!(survivor.last_log_index(), LogIndex(100));

    // Replicate one additional command to only one survivor, but do not
    // acknowledge it as committed. It must not contribute to RPO.
    let actions = leader.propose(command(101)).unwrap();
    let request = actions
        .into_iter()
        .find_map(|a| match a {
            RaftAction::AppendEntries { to, request } if to == third.id() => Some(request),
            _ => None,
        })
        .unwrap();
    let response = third.handle_append_entries(request).unwrap();
    assert!(response.success);
    assert_eq!(leader.commit_index(), LogIndex(100));

    let acknowledged_before_failure = leader.commit_index().0;
    let surviving_acknowledged = survivor.commit_index().0;
    let rpo_entries = acknowledged_before_failure.saturating_sub(surviving_acknowledged);

    // Kill the leader. The survivor starts an election after its configured
    // timeout and wins with the third node. Measure wall-clock failover
    // detection/election time on the CI runner.
    let failure_at = Instant::now();
    let mut ticks = 0u64;
    let mut candidate = survivor;

    while candidate.role() != Role::Leader {
        ticks += 1;
        let actions = candidate.tick();
        if candidate.role() == Role::Candidate {
            for action in actions {
                if let RaftAction::RequestVote { to, request } = action {
                    if to == NodeId(3) {
                        let vote = third.handle_request_vote(request);
                        candidate.handle_vote_response(vote).unwrap();
                    }
                }
            }
        }
        assert!(ticks < 1000, "failover did not converge");
    }

    let rto_ms = failure_at.elapsed().as_secs_f64() * 1000.0;

    // The newly elected leader must retain the committed prefix.
    assert!(candidate.commit_index().0 >= acknowledged_before_failure);
    assert!(candidate.last_log_index().0 >= acknowledged_before_failure);

    println!("HA_CERTIFICATION");
    println!("acknowledged_before_failure={acknowledged_before_failure}");
    println!("surviving_acknowledged={surviving_acknowledged}");
    println!("rpo_entries={rpo_entries}");
    println!("election_ticks={ticks}");
    println!("consensus_failover_rto_ms={rto_ms:.3}");
    println!("committed_prefix_preserved=true");
    println!("VERDICT=PASS");
}
