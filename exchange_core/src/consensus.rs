use std::collections::{BTreeMap, BTreeSet};

use crate::command::OrderCommand;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct NodeId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct Term(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct LogIndex(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Follower,
    Candidate,
    Leader,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LogEntry {
    pub index: LogIndex,
    pub term: Term,
    pub command: OrderCommand,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RequestVote {
    pub term: Term,
    pub candidate_id: NodeId,
    pub last_log_index: LogIndex,
    pub last_log_term: Term,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VoteResponse {
    pub term: Term,
    pub voter_id: NodeId,
    pub granted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppendEntries {
    pub term: Term,
    pub leader_id: NodeId,
    pub prev_log_index: LogIndex,
    pub prev_log_term: Term,
    pub entries: Vec<LogEntry>,
    pub leader_commit: LogIndex,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AppendResponse {
    pub term: Term,
    pub follower_id: NodeId,
    pub success: bool,
    pub match_index: LogIndex,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RaftAction {
    RequestVote { to: NodeId, request: RequestVote },
    AppendEntries { to: NodeId, request: AppendEntries },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConsensusError {
    InvalidMembership,
    NotLeader { leader: Option<NodeId> },
    StaleTerm,
    InvalidAppend,
}

#[derive(Debug)]
pub struct RaftNode {
    id: NodeId,
    members: Vec<NodeId>,
    current_term: Term,
    voted_for: Option<NodeId>,
    role: Role,
    leader_id: Option<NodeId>,
    log: Vec<LogEntry>,
    commit_index: LogIndex,
    last_applied: LogIndex,
    next_index: BTreeMap<NodeId, LogIndex>,
    match_index: BTreeMap<NodeId, LogIndex>,
    votes_received: BTreeSet<NodeId>,
    election_elapsed: u64,
    election_timeout: u64,
    heartbeat_elapsed: u64,
    heartbeat_interval: u64,
}

impl RaftNode {
    pub fn new(
        id: NodeId,
        mut members: Vec<NodeId>,
        election_timeout: u64,
        heartbeat_interval: u64,
    ) -> Result<Self, ConsensusError> {
        members.sort_unstable();
        members.dedup();
        if members.len() < 3
            || members.len().is_multiple_of(2)
            || !members.contains(&id)
            || election_timeout == 0
            || heartbeat_interval == 0
        {
            return Err(ConsensusError::InvalidMembership);
        }

        Ok(Self {
            id,
            members,
            current_term: Term(0),
            voted_for: None,
            role: Role::Follower,
            leader_id: None,
            log: Vec::new(),
            commit_index: LogIndex(0),
            last_applied: LogIndex(0),
            next_index: BTreeMap::new(),
            match_index: BTreeMap::new(),
            votes_received: BTreeSet::new(),
            election_elapsed: 0,
            election_timeout,
            heartbeat_elapsed: 0,
            heartbeat_interval,
        })
    }

    pub fn id(&self) -> NodeId {
        self.id
    }

    pub fn term(&self) -> Term {
        self.current_term
    }

    pub fn role(&self) -> Role {
        self.role
    }

    pub fn leader_id(&self) -> Option<NodeId> {
        self.leader_id
    }

    pub fn commit_index(&self) -> LogIndex {
        self.commit_index
    }

    pub fn last_applied(&self) -> LogIndex {
        self.last_applied
    }

    pub fn last_log_index(&self) -> LogIndex {
        LogIndex(self.log.len() as u64)
    }

    pub fn last_log_term(&self) -> Term {
        self.log.last().map_or(Term(0), |entry| entry.term)
    }

    pub fn log(&self) -> &[LogEntry] {
        &self.log
    }

    pub fn majority(&self) -> usize {
        self.members.len() / 2 + 1
    }

    pub fn tick(&mut self) -> Vec<RaftAction> {
        self.election_elapsed = self.election_elapsed.saturating_add(1);

        if self.role == Role::Leader {
            self.heartbeat_elapsed = self.heartbeat_elapsed.saturating_add(1);
            if self.heartbeat_elapsed >= self.heartbeat_interval {
                self.heartbeat_elapsed = 0;
                return self.heartbeat_actions();
            }
            return Vec::new();
        }

        if self.election_elapsed >= self.election_timeout {
            return self.start_election();
        }

        Vec::new()
    }

    pub fn start_election(&mut self) -> Vec<RaftAction> {
        self.current_term = Term(
            self.current_term
                .0
                .checked_add(1)
                .expect("raft term exhausted"),
        );
        self.role = Role::Candidate;
        self.leader_id = None;
        self.voted_for = Some(self.id);
        self.votes_received.clear();
        self.votes_received.insert(self.id);
        self.election_elapsed = 0;

        let request = RequestVote {
            term: self.current_term,
            candidate_id: self.id,
            last_log_index: self.last_log_index(),
            last_log_term: self.last_log_term(),
        };

        self.members
            .iter()
            .copied()
            .filter(|peer| *peer != self.id)
            .map(|to| RaftAction::RequestVote { to, request })
            .collect()
    }

    pub fn handle_request_vote(&mut self, request: RequestVote) -> VoteResponse {
        if request.term < self.current_term {
            return VoteResponse {
                term: self.current_term,
                voter_id: self.id,
                granted: false,
            };
        }

        if request.term > self.current_term {
            self.become_follower(request.term, None);
        }

        let candidate_is_up_to_date = request.last_log_term > self.last_log_term()
            || (request.last_log_term == self.last_log_term()
                && request.last_log_index >= self.last_log_index());

        let can_vote = self.voted_for.is_none() || self.voted_for == Some(request.candidate_id);
        let granted = can_vote && candidate_is_up_to_date;

        if granted {
            self.voted_for = Some(request.candidate_id);
            self.election_elapsed = 0;
        }

        VoteResponse {
            term: self.current_term,
            voter_id: self.id,
            granted,
        }
    }

    pub fn handle_vote_response(
        &mut self,
        response: VoteResponse,
    ) -> Result<Vec<RaftAction>, ConsensusError> {
        if response.term < self.current_term {
            return Err(ConsensusError::StaleTerm);
        }
        if response.term > self.current_term {
            self.become_follower(response.term, None);
            return Ok(Vec::new());
        }
        if self.role != Role::Candidate {
            return Ok(Vec::new());
        }

        if response.granted {
            self.votes_received.insert(response.voter_id);
            if self.votes_received.len() >= self.majority() {
                return Ok(self.become_leader());
            }
        }
        Ok(Vec::new())
    }

    pub fn propose(&mut self, command: OrderCommand) -> Result<Vec<RaftAction>, ConsensusError> {
        if self.role != Role::Leader {
            return Err(ConsensusError::NotLeader {
                leader: self.leader_id,
            });
        }

        let entry = LogEntry {
            index: LogIndex(self.log.len() as u64 + 1),
            term: self.current_term,
            command,
        };
        self.log.push(entry);
        Ok(self.replication_actions())
    }

    pub fn handle_append_entries(
        &mut self,
        request: AppendEntries,
    ) -> Result<AppendResponse, ConsensusError> {
        if request.term < self.current_term {
            return Ok(AppendResponse {
                term: self.current_term,
                follower_id: self.id,
                success: false,
                match_index: self.last_log_index(),
            });
        }

        if request.term > self.current_term || self.role != Role::Follower {
            self.become_follower(request.term, Some(request.leader_id));
        } else {
            self.leader_id = Some(request.leader_id);
            self.election_elapsed = 0;
        }

        if request.prev_log_index.0 > self.last_log_index().0 {
            return Ok(AppendResponse {
                term: self.current_term,
                follower_id: self.id,
                success: false,
                match_index: self.last_log_index(),
            });
        }

        if request.prev_log_index.0 > 0 {
            let local = self.log[(request.prev_log_index.0 - 1) as usize];
            if local.term != request.prev_log_term {
                self.log.truncate((request.prev_log_index.0 - 1) as usize);
                return Ok(AppendResponse {
                    term: self.current_term,
                    follower_id: self.id,
                    success: false,
                    match_index: self.last_log_index(),
                });
            }
        }

        for entry in request.entries {
            let position = entry.index.0.saturating_sub(1) as usize;
            if position < self.log.len() {
                if self.log[position].term != entry.term
                    || self.log[position].command != entry.command
                {                    self.log.truncate(position);
                    self.log.push(entry);
                }
            } else if position == self.log.len() {
                self.log.push(entry);
            } else {
                return Err(ConsensusError::InvalidAppend);
            }
        }

        let new_commit = request.leader_commit.min(self.last_log_index());
        if new_commit > self.commit_index {
            self.commit_index = new_commit;
        }

        Ok(AppendResponse {
            term: self.current_term,
            follower_id: self.id,
            success: true,
            match_index: self.last_log_index(),
        })
    }

    pub fn handle_append_response(
        &mut self,
        peer: NodeId,
        response: AppendResponse,
    ) -> Result<Vec<RaftAction>, ConsensusError> {
        if response.term < self.current_term {
            return Err(ConsensusError::StaleTerm);
        }
        if response.term > self.current_term {
            self.become_follower(response.term, None);
            return Ok(Vec::new());
        }
        if self.role != Role::Leader {
            return Ok(Vec::new());
        }

        if response.success {
            self.match_index.insert(peer, response.match_index);
            self.next_index
                .insert(peer, LogIndex(response.match_index.0 + 1));
            self.advance_commit();
            Ok(Vec::new())
        } else {
            let current = self.next_index.get(&peer).copied().unwrap_or(LogIndex(1));
            let retry_index = LogIndex(current.0.saturating_sub(1).max(1));
            self.next_index.insert(peer, retry_index);
            Ok(self.replication_for(peer))
        }
    }

    pub fn take_committed(&mut self) -> Vec<OrderCommand> {
        let mut commands = Vec::new();
        while self.last_applied < self.commit_index {
            let next = LogIndex(self.last_applied.0 + 1);
            let entry = self.log[(next.0 - 1) as usize];
            commands.push(entry.command);
            self.last_applied = next;
        }
        commands
    }

    fn become_follower(&mut self, term: Term, leader: Option<NodeId>) {
        self.current_term = term;
        self.role = Role::Follower;
        self.voted_for = None;
        self.leader_id = leader;
        self.votes_received.clear();
        self.election_elapsed = 0;
        self.heartbeat_elapsed = 0;
    }

    fn become_leader(&mut self) -> Vec<RaftAction> {
        self.role = Role::Leader;
        self.leader_id = Some(self.id);
        self.votes_received.clear();
        self.election_elapsed = 0;
        self.heartbeat_elapsed = 0;
        self.next_index.clear();
        self.match_index.clear();

        let next = LogIndex(self.last_log_index().0 + 1);
        for peer in self.members.iter().copied().filter(|peer| *peer != self.id) {
            self.next_index.insert(peer, next);
            self.match_index.insert(peer, LogIndex(0));
        }
        self.match_index.insert(self.id, self.last_log_index());

        self.heartbeat_actions()
    }

    fn heartbeat_actions(&self) -> Vec<RaftAction> {
        self.members
            .iter()
            .copied()
            .filter(|peer| *peer != self.id)
            .map(|to| RaftAction::AppendEntries {
                to,
                request: self.append_request_for(to),
            })
            .collect()
    }

    fn replication_actions(&self) -> Vec<RaftAction> {
        self.members
            .iter()
            .copied()
            .filter(|peer| *peer != self.id)
            .map(|to| RaftAction::AppendEntries {
                to,
                request: self.append_request_for(to),
            })
            .collect()
    }

    fn replication_for(&self, peer: NodeId) -> Vec<RaftAction> {
        vec![RaftAction::AppendEntries {
            to: peer,
            request: self.append_request_for(peer),
        }]
    }

    fn append_request_for(&self, peer: NodeId) -> AppendEntries {
        let next = self.next_index.get(&peer).copied().unwrap_or(LogIndex(1));
        let prev = LogIndex(next.0.saturating_sub(1));
        let prev_term = if prev.0 == 0 {
            Term(0)
        } else {
            self.log[(prev.0 - 1) as usize].term
        };
        let entries = self
            .log
            .iter()
            .filter(|entry| entry.index.0 >= next.0)
            .copied()
            .collect();

        AppendEntries {
            term: self.current_term,
            leader_id: self.id,
            prev_log_index: prev,
            prev_log_term: prev_term,
            entries,
            leader_commit: self.commit_index,
        }
    }

    fn advance_commit(&mut self) {
        for candidate in (self.commit_index.0 + 1)..=self.last_log_index().0 {
            let replicated = self
                .match_index
                .values()
                .filter(|index| index.0 >= candidate)
                .count()
                + 1;
            if replicated >= self.majority()
                && self.log[(candidate - 1) as usize].term == self.current_term
            {
                self.commit_index = LogIndex(candidate);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::{NewOrder, OrderCommand, OrderSide};
    use crate::order::ClientOrderId;

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

    #[test]
    fn election_requires_majority_and_fences_stale_vote() {
        let mut leader = RaftNode::new(NodeId(1), members(), 3, 1).unwrap();
        let actions = leader.start_election();
        assert_eq!(leader.role(), Role::Candidate);
        assert_eq!(actions.len(), 2);

        let response = VoteResponse {
            term: leader.term(),
            voter_id: NodeId(2),
            granted: true,
        };
        let actions = leader.handle_vote_response(response).unwrap();
        assert_eq!(leader.role(), Role::Leader);
        assert_eq!(actions.len(), 2);

        let stale = leader.handle_vote_response(VoteResponse {
            term: Term(0),
            voter_id: NodeId(3),
            granted: true,
        });
        assert_eq!(stale, Err(ConsensusError::StaleTerm));
    }

    #[test]
    fn follower_rejects_stale_leader_and_accepts_current_log() {
        let mut follower = RaftNode::new(NodeId(2), members(), 3, 1).unwrap();
        follower.start_election();
        let response = follower
            .handle_append_entries(AppendEntries {
                term: Term(0),
                leader_id: NodeId(1),
                prev_log_index: LogIndex(0),
                prev_log_term: Term(0),
                entries: Vec::new(),
                leader_commit: LogIndex(0),
            })
            .unwrap();
        assert!(!response.success);

        let response = follower
            .handle_append_entries(AppendEntries {
                term: Term(1),
                leader_id: NodeId(1),
                prev_log_index: LogIndex(0),
                prev_log_term: Term(0),
                entries: vec![LogEntry {
                    index: LogIndex(1),
                    term: Term(1),
                    command: command(1),
                }],
                leader_commit: LogIndex(1),
            })
            .unwrap();
        assert!(response.success);
        assert_eq!(follower.commit_index(), LogIndex(1));
        assert_eq!(follower.take_committed(), vec![command(1)]);
    }

    #[test]
    fn leader_replicates_and_commits_on_majority() {
        let mut leader = RaftNode::new(NodeId(1), members(), 3, 1).unwrap();
        leader.start_election();
        leader
            .handle_vote_response(VoteResponse {
                term: Term(1),
                voter_id: NodeId(2),
                granted: true,
            })
            .unwrap();
        assert_eq!(leader.role(), Role::Leader);

        let actions = leader.propose(command(9)).unwrap();
        assert_eq!(actions.len(), 2);

        let append_to_two = match &actions[0] {
            RaftAction::AppendEntries { request, .. } => request.clone(),
            _ => unreachable!(),
        };
        let response = AppendResponse {
            term: Term(1),
            follower_id: NodeId(2),
            success: true,
            match_index: append_to_two.entries.last().unwrap().index,
        };
        leader.handle_append_response(NodeId(2), response).unwrap();

        assert_eq!(leader.commit_index(), LogIndex(1));
        assert_eq!(leader.take_committed(), vec![command(9)]);
    }

    #[test]
    fn conflicting_suffix_is_replaced() {
        let mut follower = RaftNode::new(NodeId(2), members(), 3, 1).unwrap();
        let old = LogEntry {
            index: LogIndex(1),
            term: Term(1),
            command: command(1),
        };
        follower
            .handle_append_entries(AppendEntries {
                term: Term(1),
                leader_id: NodeId(1),
                prev_log_index: LogIndex(0),
                prev_log_term: Term(0),
                entries: vec![old],
                leader_commit: LogIndex(0),
            })
            .unwrap();

        let replacement = LogEntry {
            index: LogIndex(1),
            term: Term(2),
            command: command(2),
        };
        follower
            .handle_append_entries(AppendEntries {
                term: Term(2),
                leader_id: NodeId(1),
                prev_log_index: LogIndex(0),
                prev_log_term: Term(0),
                entries: vec![replacement],
                leader_commit: LogIndex(0),
            })
            .unwrap();

        assert_eq!(follower.log()[0].command, command(2));
        assert_eq!(follower.log()[0].term, Term(2));
    }

    #[test]
    fn candidate_steps_down_on_higher_term() {
        let mut node = RaftNode::new(NodeId(1), members(), 3, 1).unwrap();
        node.start_election();
        let actions = node
            .handle_vote_response(VoteResponse {
                term: Term(2),
                voter_id: NodeId(2),
                granted: false,
            })
            .unwrap();
        assert!(actions.is_empty());
        assert_eq!(node.role(), Role::Follower);
        assert_eq!(node.term(), Term(2));
    }
}
