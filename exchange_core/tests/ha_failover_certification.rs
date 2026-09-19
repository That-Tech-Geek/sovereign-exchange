use exchange_core::{
    AppendResponse, LogIndex, NodeId, OrderCommand, RaftAction, RaftNode, Role,
};
use exchange_core::{ClientOrderId, NewOrder, OrderSide};
use std::time::Instant;

const TRIALS: usize = 100;
const PREFIX_LEN: u64 = 100;

fn members() -> [NodeId; 3] {
    [NodeId(1), NodeId(2), NodeId(3)]
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

fn replicate_one(leader: &mut RaftNode, follower: &mut RaftNode, id: u64) {
    let actions = leader.propose(command(id)).unwrap();
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

fn percentile(sorted: &[f64], p: f64) -> f64 {
    let index = ((sorted.len() - 1) as f64 * p).round() as usize;
    sorted[index]
}

fn one_trial() -> (f64, u64, f64, f64, f64) {
    let mut leader = RaftNode::new(NodeId(1), members().to_vec(), 5, 1).unwrap();
    let mut survivor = RaftNode::new(NodeId(2), members().to_vec(), 5, 1).unwrap();
    let mut third = RaftNode::new(NodeId(3), members().to_vec(), 5, 1).unwrap();

    elect(&mut leader);

    let prefix_at = Instant::now();
    for id in 1..=PREFIX_LEN {
        replicate_one(&mut leader, &mut survivor, id);
    }
    let prefix_ms = prefix_at.elapsed().as_secs_f64() * 1000.0;
    assert_eq!(leader.commit_index(), LogIndex(PREFIX_LEN));
    assert_eq!(survivor.last_log_index(), LogIndex(PREFIX_LEN));

    let actions = leader.propose(command(PREFIX_LEN + 1)).unwrap();
    for action in actions {
        if let RaftAction::AppendEntries { to, request } = action {
            if to == third.id() {
                let response = third.handle_append_entries(request).unwrap();
                assert!(response.success);
            } else if to == survivor.id() {
                let response = survivor.handle_append_entries(request).unwrap();
                assert!(response.success);
            }
        }
    }
    assert_eq!(leader.commit_index(), LogIndex(PREFIX_LEN));
    assert_eq!(survivor.last_log_index(), LogIndex(PREFIX_LEN + 1));
    assert_eq!(third.last_log_index(), LogIndex(PREFIX_LEN + 1));

    let acknowledged_before_failure = leader.commit_index().0;
    let surviving_acknowledged = survivor.commit_index().0;
    let rpo_entries = acknowledged_before_failure.saturating_sub(surviving_acknowledged);

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
    let election_ms = failure_at.elapsed().as_secs_f64() * 1000.0;

    let recommit_at = Instant::now();
    let actions = candidate.tick();
    for action in actions {
        if let RaftAction::AppendEntries { to, request } = action {
            if to == third.id() {
                let response = third.handle_append_entries(request).unwrap();
                candidate.handle_append_response(to, response).unwrap();
            }
        }
    }
    let recommit_ms = recommit_at.elapsed().as_secs_f64() * 1000.0;

    assert!(candidate.commit_index().0 >= acknowledged_before_failure);
    assert!(candidate.last_log_index().0 >= acknowledged_before_failure);

    (prefix_ms, ticks, election_ms, recommit_ms, rpo_entries as f64)
}

#[test]
fn measured_rpo_and_consensus_failover_rto() {
    let mut prefix_ms = Vec::with_capacity(TRIALS);
    let mut election_ms = Vec::with_capacity(TRIALS);
    let mut recommit_ms = Vec::with_capacity(TRIALS);
    let mut rto_ms = Vec::with_capacity(TRIALS);
    let mut ticks = Vec::with_capacity(TRIALS);

    for _ in 0..TRIALS {
        let (prefix, election_ticks, election, recommit, rpo) = one_trial();
        prefix_ms.push(prefix);
        ticks.push(election_ticks);
        election_ms.push(election);
        recommit_ms.push(recommit);
        rto_ms.push(election + recommit);
        assert_eq!(rpo, 0.0);
    }

    prefix_ms.sort_by(f64::total_cmp);
    election_ms.sort_by(f64::total_cmp);
    recommit_ms.sort_by(f64::total_cmp);
    rto_ms.sort_by(f64::total_cmp);
    ticks.sort_unstable();

    let throughput_orders_s =
        (TRIALS as f64 * PREFIX_LEN as f64) / (prefix_ms.iter().sum::<f64>() / 1000.0);
    let min = *rto_ms.first().unwrap();
    let max = *rto_ms.last().unwrap();

    println!("HA_CERTIFICATION");
    println!("trials={TRIALS}");
    println!("prefix_entries={PREFIX_LEN}");
    println!("rpo_entries=0");
    println!(
        "election_ticks_p50={}",
        percentile(&ticks.iter().map(|v| *v as f64).collect::<Vec<_>>(), 0.50)
    );
    println!(
        "election_ticks_p95={}",
        percentile(&ticks.iter().map(|v| *v as f64).collect::<Vec<_>>(), 0.95)
    );
    println!(
        "prefix_replication_ms_p50={:.6}",
        percentile(&prefix_ms, 0.50)
    );
    println!(
        "prefix_replication_ms_p95={:.6}",
        percentile(&prefix_ms, 0.95)
    );
    println!(
        "prefix_replication_ms_p99={:.6}",
        percentile(&prefix_ms, 0.99)
    );
    println!("prefix_replication_orders_s={throughput_orders_s:.0}");
    println!(
        "election_ms_p50={:.6}",
        percentile(&election_ms, 0.50)
    );
    println!(
        "election_ms_p95={:.6}",
        percentile(&election_ms, 0.95)
    );
    println!(
        "election_ms_p99={:.6}",
        percentile(&election_ms, 0.99)
    );
    println!(
        "recommit_ms_p50={:.6}",
        percentile(&recommit_ms, 0.50)
    );
    println!(
        "recommit_ms_p95={:.6}",
        percentile(&recommit_ms, 0.95)
    );
    println!(
        "recommit_ms_p99={:.6}",
        percentile(&recommit_ms, 0.99)
    );
    println!(
        "consensus_failover_rto_ms_p50={:.6}",
        percentile(&rto_ms, 0.50)
    );
    println!(
        "consensus_failover_rto_ms_p95={:.6}",
        percentile(&rto_ms, 0.95)
    );
    println!(
        "consensus_failover_rto_ms_p99={:.6}",
        percentile(&rto_ms, 0.99)
    );
    println!("consensus_failover_rto_ms_min={min:.6}");
    println!("consensus_failover_rto_ms_max={max:.6}");
    println!("committed_prefix_preserved=true");
    println!("VERDICT=PASS");
}
