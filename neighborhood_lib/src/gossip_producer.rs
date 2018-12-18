// Copyright (c) 2017-2018, Substratum LLC (https://substratum.net) and/or its affiliates. All rights reserved.
use gossip::Gossip;
use sub_lib::cryptde::Key;
use neighborhood_database::NeighborhoodDatabase;
use gossip::GossipBuilder;
use sub_lib::logger::Logger;
use neighborhood_database::NodeRecord;

static MINIMUM_NEIGHBORS: usize = 3;

pub trait GossipProducer {
    fn produce (&self, database: &NeighborhoodDatabase, target: &Key) -> Gossip;
}

pub struct GossipProducerReal {
    _logger: Logger,
}

impl GossipProducer for GossipProducerReal {
    /*
        `produce`
            the purpose of `produce` is to convert the raw neighborhood from the DB into a Gossip message for a target node
            the Gossip that `produce` returns includes the entire neighborhood, but masks the IP addresses of nodes that
            are not directly connected to `target`. it also filters out connections from any node to any bootstrap_node
        params:
            `database`: the DB that contains the whole neighborhood
            `target`: the node to produce the gossip for
                allows `produce` to determine which ip addrs to mask/reveal, based on which other nodes `target` is connected to (in either direction)
        returns:
            a Gossip message representing the current neighborhood for a target node
    */
    fn produce(&self, database: &NeighborhoodDatabase, target: &Key) -> Gossip {
        let target_node_ref = match database.node_by_key (target) {
            Some (node_ref) => node_ref,
            None => panic! ("Target node {:?} not in NeighborhoodDatabase", target)
        };

        let introducees = self.choose_introductions(database, target_node_ref);
        let builder = database.keys ().into_iter ()
            .fold (GossipBuilder::new (), |so_far, key_ref| {
                let node_record_ref = database.node_by_key (key_ref).expect ("Key magically disappeared");
                let reveal_node_addr =
                    node_record_ref.has_neighbor (target_node_ref.public_key ()) ||
                    target_node_ref.has_neighbor (node_record_ref.public_key ()) ||
                    introducees.contains(&key_ref);
                so_far.node (node_record_ref, reveal_node_addr)
            });
        let builder = database.keys ().into_iter ().fold (builder, |so_far_outer, key_ref| {
            database.node_by_key (key_ref).expect ("Key magically disappeared").neighbors ().iter ()
                .filter(|neighbor| !database.node_by_key(neighbor).expect("Key magically disappeared").is_bootstrap_node())
                .fold (so_far_outer, |so_far_inner, neighbor_ref| {
                so_far_inner.neighbor_pair (key_ref, neighbor_ref)
            })
        });

        builder.build ()
    }
}

impl GossipProducerReal {
    pub fn new() -> GossipProducerReal {
        GossipProducerReal { _logger: Logger::new ("GossipProducerReal") }
    }

    pub fn choose_introductions<'a>(&self, database: &'a NeighborhoodDatabase, target: &NodeRecord) -> Vec<&'a Key> {
        let target_standard_neighbors = target.neighbors().iter()
            .filter(|key| match database.node_by_key(key) {
                Some(node) => !node.is_bootstrap_node(),
                None => unimplemented!() // we don't know this node, so we should assume it is not a bootstrap node
            })
            .count();

        if !target.is_bootstrap_node() && database.root().neighbors().contains(target.public_key()) && target_standard_neighbors < MINIMUM_NEIGHBORS {
            let mut possible_introducees: Vec<&Key> = database.root()
                .neighbors().iter()
                .filter(|key| !target.neighbors().contains(key))
                .filter(|key| target.public_key() != *key)
                .filter(|key| !database.node_by_key(key).expect("Key magically disappeared").is_bootstrap_node())
                .collect();

            possible_introducees.sort_by(|l, r|
                database.node_by_key(l).expect("Key magically disappeared").neighbors().len()
                    .cmp(&database.node_by_key(r).expect("Key magically disappeared").neighbors().len())
            );

            possible_introducees.into_iter().take(MINIMUM_NEIGHBORS - target_standard_neighbors).collect()
        } else {
            vec!()
        }
    }
}

#[cfg (test)]
mod tests {
    use super::*;
    use neighborhood_test_utils::*;
    use gossip::GossipNodeRecord;
    use test_utils::test_utils::cryptde;
    use sub_lib::cryptde_null::CryptDENull;
    use test_utils::test_utils::assert_contains;

    #[test]
    #[should_panic(expected="Target node AgMEBQ not in NeighborhoodDatabase")]
    fn produce_fails_for_target_not_in_database() {
        let this_node = make_node_record(1234, true, false);
        let target_node = make_node_record(2345, true, false);
        let database = NeighborhoodDatabase::new(this_node.public_key(), this_node.node_addr_opt().as_ref().unwrap(), this_node.is_bootstrap_node(), cryptde ());

        let subject = GossipProducerReal::new();

        subject.produce(&database, target_node.public_key());
    }

    #[test]
    fn database_produces_gossip_with_standard_gossip_handler_and_well_connected_target () {
        let mut this_node = make_node_record(1234, true, false);
        let mut first_neighbor = make_node_record(2345, true, false);
        let second_neighbor = make_node_record(3456, true, true);
        let mut target = make_node_record (4567, false, false);
        this_node.neighbors_mut().push (first_neighbor.public_key ().clone ());
        this_node.neighbors_mut().push (second_neighbor.public_key ().clone ());
        first_neighbor.neighbors_mut().push (second_neighbor.public_key ().clone ());
        first_neighbor.neighbors_mut().push (target.public_key ().clone ());
        target.neighbors_mut().push (second_neighbor.public_key ().clone ());

        let mut database = NeighborhoodDatabase::new(this_node.public_key(),
                                                     this_node.node_addr_opt().as_ref().unwrap(), this_node.is_bootstrap_node(), &CryptDENull::from(this_node.public_key()));

        database.add_node(&first_neighbor).unwrap();
        database.add_node(&second_neighbor).unwrap();
        database.add_node(&target).unwrap();
        database.add_neighbor(this_node.public_key(), first_neighbor.public_key()).unwrap();
        database.add_neighbor(this_node.public_key(), second_neighbor.public_key()).unwrap();
        database.add_neighbor(first_neighbor.public_key(), second_neighbor.public_key()).unwrap();
        database.add_neighbor(first_neighbor.public_key (), target.public_key ()).unwrap ();
        database.add_neighbor (target.public_key (), second_neighbor.public_key ()).unwrap ();
        let subject = GossipProducerReal::new();

        let result = subject.produce(&database, target.public_key ());

        assert_contains (&result.node_records, &GossipNodeRecord::from(&this_node, false));
        assert_contains (&result.node_records, &GossipNodeRecord::from(&first_neighbor, true));
        assert_contains (&result.node_records, &GossipNodeRecord::from(&second_neighbor, true));
        assert_contains (&result.node_records, &GossipNodeRecord::from(&target, false));
        assert_eq!(result.node_records.len(), 4);
        let neighbor_keys: Vec<(Key, Key)> = result.neighbor_pairs.iter().map(|neighbor_relationship| {
            let from_idx = neighbor_relationship.from;
            let to_idx = neighbor_relationship.to;
            let from_key: Key = result.node_records.get(from_idx as usize).unwrap().inner.public_key.clone();
            let to_key: Key = result.node_records.get(to_idx as usize).unwrap().inner.public_key.clone();
            (from_key, to_key)
        }).collect();
        assert_eq!(neighbor_keys.len(), 2);
        assert_contains (&neighbor_keys, &(this_node.public_key().clone(),
                                            first_neighbor.public_key().clone()));
        assert_contains (&neighbor_keys, &(first_neighbor.public_key().clone(),
                                            target.public_key().clone()));
    }

    #[test]
    fn database_produces_gossip_with_badly_connected_target () {
        let mut this_node = make_node_record(1234, true, false);
        let mut first_neighbor = make_node_record(2345, true, false);
        let second_neighbor = make_node_record(3456, true, true);
        let target = make_node_record (4567, false, false);
        this_node.neighbors_mut().push (first_neighbor.public_key ().clone ());
        this_node.neighbors_mut().push (second_neighbor.public_key ().clone ());
        first_neighbor.neighbors_mut().push (second_neighbor.public_key ().clone ());
        let mut database = NeighborhoodDatabase::new(this_node.public_key(),
                                                     this_node.node_addr_opt().as_ref().unwrap(), this_node.is_bootstrap_node(), &CryptDENull::from(this_node.public_key()));
        database.add_node(&first_neighbor).unwrap();
        database.add_node(&second_neighbor).unwrap();
        database.add_node(&target).unwrap();
        database.add_neighbor(this_node.public_key(), first_neighbor.public_key()).unwrap();
        database.add_neighbor(this_node.public_key(), second_neighbor.public_key()).unwrap();
        database.add_neighbor(first_neighbor.public_key(), second_neighbor.public_key()).unwrap();
        let subject = GossipProducerReal::new();

        let result = subject.produce(&database, target.public_key ());

        assert_contains (&result.node_records, &GossipNodeRecord::from(&this_node, false));
        assert_contains (&result.node_records, &GossipNodeRecord::from(&first_neighbor, false));
        assert_contains (&result.node_records, &GossipNodeRecord::from(&second_neighbor, false));
        assert_contains (&result.node_records, &GossipNodeRecord::from(&target, false));
        assert_eq!(result.node_records.len(), 4);
        let neighbor_keys: Vec<(Key, Key)> = result.neighbor_pairs.iter().map(|neighbor_relationship| {
            let from_idx = neighbor_relationship.from;
            let to_idx = neighbor_relationship.to;
            let from_key: Key = result.node_records.get(from_idx as usize).unwrap().inner.public_key.clone();
            let to_key: Key = result.node_records.get(to_idx as usize).unwrap().inner.public_key.clone();
            (from_key, to_key)
        }).collect();
        assert_eq!(neighbor_keys.len(), 1);
        assert_contains (&neighbor_keys, &(this_node.public_key().clone(), first_neighbor.public_key().clone()));
    }

    #[test]
    fn gossip_producer_filters_out_target_connections_to_bootstrap_nodes() { //but keeps target connections from bootstrap nodes
        let mut this_node = make_node_record(1234, true, false);
        let mut bootstrap = make_node_record(3456, true, true);
        let mut target = make_node_record (4567, false, false);
        this_node.neighbors_mut().push (bootstrap.public_key ().clone ());
        bootstrap.neighbors_mut().push (target.public_key ().clone ());
        target.neighbors_mut().push (bootstrap.public_key ().clone ());
        let mut database = NeighborhoodDatabase::new(this_node.public_key(),
                                                     this_node.node_addr_opt().as_ref().unwrap(), this_node.is_bootstrap_node(), &CryptDENull::from(this_node.public_key()));
        database.add_node(&bootstrap).unwrap();
        database.add_node(&target).unwrap();
        database.add_neighbor(this_node.public_key(), bootstrap.public_key()).unwrap();
        database.add_neighbor (target.public_key (), bootstrap.public_key ()).unwrap ();
        database.add_neighbor (bootstrap.public_key (), target.public_key ()).unwrap ();
        let subject = GossipProducerReal::new();

        let result = subject.produce(&database, target.public_key ());

        assert_contains (&result.node_records, &GossipNodeRecord::from(&this_node, false));
        assert_contains (&result.node_records, &GossipNodeRecord::from(&bootstrap, true));
        assert_contains (&result.node_records, &GossipNodeRecord::from(&target, false));
        assert_eq!(result.node_records.len(), 3);
        let neighbor_keys: Vec<(Key, Key)> = result.neighbor_pairs.iter().map(|neighbor_relationship| {
            let from_idx = neighbor_relationship.from;
            let to_idx = neighbor_relationship.to;
            let from_key: Key = result.node_records.get(from_idx as usize).unwrap().inner.public_key.clone();
            let to_key: Key = result.node_records.get(to_idx as usize).unwrap().inner.public_key.clone();
            (from_key, to_key)
        }).collect();
        assert_eq!(neighbor_keys.contains(&(bootstrap.public_key().clone(),
                                            target.public_key().clone())), true, "{:?}", neighbor_keys);
        assert_eq!(neighbor_keys.len(), 1);

    }

    #[test]
    fn gossip_producer_masks_ip_addrs_for_nodes_not_directly_connected_to_target() {
        let mut this_node = make_node_record(1234, true, false);
        let mut first_neighbor = make_node_record(2345, true, false);
        let second_neighbor = make_node_record(3456, true, false);
        let mut target = make_node_record (4567, false, false);
        this_node.neighbors_mut().push (first_neighbor.public_key().clone ());
        this_node.neighbors_mut().push (second_neighbor.public_key().clone ());
        first_neighbor.neighbors_mut().push(second_neighbor.public_key().clone ());
        first_neighbor.neighbors_mut().push(target.public_key().clone ());
        target.neighbors_mut().push (second_neighbor.public_key ().clone ());
        let mut database = NeighborhoodDatabase::new(this_node.public_key(),
                                                     this_node.node_addr_opt().as_ref().unwrap(), this_node.is_bootstrap_node(), &CryptDENull::from(this_node.public_key()));
        database.add_node(&first_neighbor).unwrap();
        database.add_node(&second_neighbor).unwrap();
        database.add_node(&target).unwrap();
        database.add_neighbor(this_node.public_key(), first_neighbor.public_key()).unwrap();
        database.add_neighbor(this_node.public_key(), second_neighbor.public_key()).unwrap();
        database.add_neighbor(first_neighbor.public_key(), second_neighbor.public_key()).unwrap();
        database.add_neighbor(first_neighbor.public_key (), target.public_key ()).unwrap ();
        database.add_neighbor (target.public_key (), second_neighbor.public_key ()).unwrap ();
        let subject = GossipProducerReal::new();

        let result = subject.produce(&database, target.public_key ());

        assert_contains (&result.node_records, &GossipNodeRecord::from(&this_node, false));
        assert_contains (&result.node_records, &GossipNodeRecord::from(&first_neighbor, true));
        assert_contains (&result.node_records, &GossipNodeRecord::from(&second_neighbor, true));
        assert_contains (&result.node_records, &GossipNodeRecord::from(&target, false));
        assert_eq!(result.node_records.len(), 4);
        let neighbor_connections: Vec<(GossipNodeRecord, GossipNodeRecord)> = result.neighbor_pairs.iter().map(|neighbor_relationship| {
            let from_idx = neighbor_relationship.from;
            let to_idx = neighbor_relationship.to;
            let from: GossipNodeRecord = result.node_records.get(from_idx as usize).unwrap().clone();
            let to: GossipNodeRecord = result.node_records.get(to_idx as usize).unwrap().clone();
            (from, to)
        }).collect();

        assert_contains (&neighbor_connections, &(GossipNodeRecord::from(&first_neighbor, true),
                                                     GossipNodeRecord::from(&target, false)));
        assert_contains (&neighbor_connections, &(GossipNodeRecord::from(&target, false),
                                                     GossipNodeRecord::from(&second_neighbor, true)));

        assert_contains (&neighbor_connections, &(GossipNodeRecord::from(&this_node, false), // node_addr of this_node is not revealed for target
                                                   GossipNodeRecord::from(&first_neighbor, true)));

        assert_contains (&neighbor_connections, &(GossipNodeRecord::from(&this_node, false),
                                                   GossipNodeRecord::from(&second_neighbor, true)));
        assert_contains (&neighbor_connections, &(GossipNodeRecord::from(&first_neighbor, true),
                                                   GossipNodeRecord::from(&second_neighbor, true)));
        assert_eq!(neighbor_connections.len(), 5);
    }

    #[test]
    fn gossip_producer_reveals_ip_addr_to_introduce_target_to_more_nodes() {
        let mut this_node = make_node_record(1234, true, false);
        let mut first_neighbor = make_node_record(2345, true, false);
        let mut second_neighbor = make_node_record(3456, true, false);
        let mut target = make_node_record (4567, true, false);
        this_node.neighbors_mut().push (first_neighbor.public_key().clone ());
        this_node.neighbors_mut().push (second_neighbor.public_key().clone ());
        this_node.neighbors_mut().push (target.public_key().clone ());
        first_neighbor.neighbors_mut().push (this_node.public_key().clone ());
        first_neighbor.neighbors_mut().push (second_neighbor.public_key().clone ());
        second_neighbor.neighbors_mut().push (first_neighbor.public_key().clone ());
        second_neighbor.neighbors_mut().push (this_node.public_key().clone ());
        target.neighbors_mut().push (this_node.public_key().clone ());
        let mut database = NeighborhoodDatabase::new(this_node.public_key(),
                                                     this_node.node_addr_opt().as_ref().unwrap(), this_node.is_bootstrap_node(), &CryptDENull::from(this_node.public_key()));
        database.add_node(&first_neighbor).unwrap();
        database.add_node(&second_neighbor).unwrap();
        database.add_node(&target).unwrap();
        database.add_neighbor(this_node.public_key(), first_neighbor.public_key()).unwrap();
        database.add_neighbor(first_neighbor.public_key(), this_node.public_key()).unwrap();
        database.add_neighbor(this_node.public_key(), second_neighbor.public_key()).unwrap();
        database.add_neighbor(second_neighbor.public_key(), this_node.public_key()).unwrap();
        database.add_neighbor(first_neighbor.public_key(), second_neighbor.public_key()).unwrap();
        database.add_neighbor(second_neighbor.public_key(), first_neighbor.public_key ()).unwrap ();
        database.add_neighbor (this_node.public_key (), target.public_key ()).unwrap ();
        database.add_neighbor (target.public_key (), this_node.public_key ()).unwrap ();

        let subject = GossipProducerReal::new();

        let result = subject.produce(&database, target.public_key ());

        assert_contains (&result.node_records, &GossipNodeRecord::from(&first_neighbor, true));
        assert_contains (&result.node_records, &GossipNodeRecord::from(&this_node, true));
        assert_contains (&result.node_records, &GossipNodeRecord::from(&second_neighbor, true));
        assert_contains (&result.node_records, &GossipNodeRecord::from(&target, false));
        assert_eq!(result.node_records.len(), 4);
    }

    #[test]
    fn gossip_producer_does_not_introduce_bootstrap_target_to_more_nodes() {
        let mut this_node = make_node_record(1234, true, false);
        let mut first_neighbor = make_node_record(2345, true, false);
        let mut second_neighbor = make_node_record(3456, true, false);
        let mut target = make_node_record (4567, true, true);
        this_node.neighbors_mut().push (first_neighbor.public_key().clone ());
        this_node.neighbors_mut().push (second_neighbor.public_key().clone ());
        this_node.neighbors_mut().push (target.public_key().clone ());
        first_neighbor.neighbors_mut().push (this_node.public_key().clone ());
        first_neighbor.neighbors_mut().push (second_neighbor.public_key().clone ());
        second_neighbor.neighbors_mut().push (first_neighbor.public_key().clone ());
        second_neighbor.neighbors_mut().push (this_node.public_key().clone ());
        target.neighbors_mut().push (this_node.public_key().clone ());
        let mut database = NeighborhoodDatabase::new(this_node.public_key(),
                                                     this_node.node_addr_opt().as_ref().unwrap(), this_node.is_bootstrap_node(), &CryptDENull::from(this_node.public_key()));
        database.add_node(&first_neighbor).unwrap();
        database.add_node(&second_neighbor).unwrap();
        database.add_node(&target).unwrap();
        database.add_neighbor(this_node.public_key(), first_neighbor.public_key()).unwrap();
        database.add_neighbor(first_neighbor.public_key(), this_node.public_key()).unwrap();
        database.add_neighbor(this_node.public_key(), second_neighbor.public_key()).unwrap();
        database.add_neighbor(second_neighbor.public_key(), this_node.public_key()).unwrap();
        database.add_neighbor(first_neighbor.public_key(), second_neighbor.public_key()).unwrap();
        database.add_neighbor(second_neighbor.public_key(), first_neighbor.public_key ()).unwrap ();
        database.add_neighbor (this_node.public_key (), target.public_key ()).unwrap ();
        database.add_neighbor (target.public_key (), this_node.public_key ()).unwrap ();

        let subject = GossipProducerReal::new();

        let result = subject.produce(&database, target.public_key ());

        assert_contains (&result.node_records, &GossipNodeRecord::from(&this_node, true));
        assert_contains (&result.node_records, &GossipNodeRecord::from(&first_neighbor, false));
        assert_contains (&result.node_records, &GossipNodeRecord::from(&second_neighbor, false));
        assert_contains (&result.node_records, &GossipNodeRecord::from(&target, false));
        assert_eq!(result.node_records.len(), 4);
    }

    #[test]
    fn gossip_producer_makes_introductions_based_on_targets_number_of_connections_to_standard_nodes_only() {
        let mut this_node = make_node_record(1234, true, false);
        let mut first_neighbor = make_node_record(2345, true, false);
        let mut second_neighbor = make_node_record(3456, true, false);
        let first_bootstrap = make_node_record(5678, false, true);
        let second_bootstrap = make_node_record(6789, false, true);
        let third_bootstrap = make_node_record(7890, false, true);
        let mut target = make_node_record (4567, true, false);
        this_node.neighbors_mut().push (first_neighbor.public_key().clone ());
        this_node.neighbors_mut().push (second_neighbor.public_key().clone ());
        this_node.neighbors_mut().push (target.public_key().clone ());
        first_neighbor.neighbors_mut().push (this_node.public_key().clone ());
        first_neighbor.neighbors_mut().push (second_neighbor.public_key().clone ());
        second_neighbor.neighbors_mut().push (first_neighbor.public_key().clone ());
        second_neighbor.neighbors_mut().push (this_node.public_key().clone ());
        target.neighbors_mut().push (this_node.public_key().clone ());
        target.neighbors_mut().push (first_bootstrap.public_key().clone ());
        target.neighbors_mut().push (second_bootstrap.public_key().clone ());
        target.neighbors_mut().push (third_bootstrap.public_key().clone ());
        let mut database = NeighborhoodDatabase::new(this_node.public_key(),
                                                     this_node.node_addr_opt().as_ref().unwrap(), this_node.is_bootstrap_node(), &CryptDENull::from(this_node.public_key()));
        database.add_node(&first_neighbor).unwrap();
        database.add_node(&second_neighbor).unwrap();
        database.add_node(&target).unwrap();
        database.add_node(&first_bootstrap).unwrap();
        database.add_node(&second_bootstrap).unwrap();
        database.add_node(&third_bootstrap).unwrap();
        database.add_neighbor(this_node.public_key(), first_neighbor.public_key()).unwrap();
        database.add_neighbor(first_neighbor.public_key(), this_node.public_key()).unwrap();
        database.add_neighbor(this_node.public_key(), second_neighbor.public_key()).unwrap();
        database.add_neighbor(second_neighbor.public_key(), this_node.public_key()).unwrap();
        database.add_neighbor(first_neighbor.public_key(), second_neighbor.public_key()).unwrap();
        database.add_neighbor(second_neighbor.public_key(), first_neighbor.public_key ()).unwrap ();
        database.add_neighbor (this_node.public_key (), target.public_key ()).unwrap ();
        database.add_neighbor (target.public_key (), this_node.public_key ()).unwrap ();
        database.add_neighbor (target.public_key (), first_bootstrap.public_key ()).unwrap ();
        database.add_neighbor (target.public_key (), second_bootstrap.public_key ()).unwrap ();
        database.add_neighbor (target.public_key (), third_bootstrap.public_key ()).unwrap ();

        let subject = GossipProducerReal::new();

        let result = subject.produce(&database, target.public_key ());

        assert_contains (&result.node_records, &GossipNodeRecord::from(&this_node, true));
        assert_contains (&result.node_records, &GossipNodeRecord::from(&first_neighbor, true));
        assert_contains (&result.node_records, &GossipNodeRecord::from(&second_neighbor, true));
        assert_contains (&result.node_records, &GossipNodeRecord::from(&target, false));
        assert_contains (&result.node_records, &GossipNodeRecord::from(&first_bootstrap, false));
        assert_contains (&result.node_records, &GossipNodeRecord::from(&second_bootstrap, false));
        assert_contains (&result.node_records, &GossipNodeRecord::from(&third_bootstrap, false));
        assert_eq!(result.node_records.len(), 7);
    }

    #[test]
    fn gossip_producer_introduces_target_to_less_connected_neighbors() {
        let mut this_node = make_node_record(1234, true, false);
        let mut first_neighbor = make_node_record(2345, true, false);
        let mut second_neighbor = make_node_record(3456, true, false);
        let mut target = make_node_record (4567, true, false);
        let target_neighbor = make_node_record (5678, true, false);
        this_node.neighbors_mut().push (first_neighbor.public_key().clone ());
        this_node.neighbors_mut().push (target_neighbor.public_key().clone ());
        this_node.neighbors_mut().push (second_neighbor.public_key().clone ());
        this_node.neighbors_mut().push (target.public_key().clone ());
        first_neighbor.neighbors_mut().push (this_node.public_key().clone ());
        first_neighbor.neighbors_mut().push (second_neighbor.public_key().clone ());
        first_neighbor.neighbors_mut().push (target_neighbor.public_key().clone ());
        second_neighbor.neighbors_mut().push (first_neighbor.public_key().clone ());
        second_neighbor.neighbors_mut().push (this_node.public_key().clone ());
        target.neighbors_mut().push (this_node.public_key().clone ());
        target.neighbors_mut().push (target_neighbor.public_key().clone ());

        let mut database = NeighborhoodDatabase::new(this_node.public_key(),
                                                     this_node.node_addr_opt().as_ref().unwrap(), this_node.is_bootstrap_node(), &CryptDENull::from(this_node.public_key()));
        database.add_node(&first_neighbor).unwrap();
        database.add_node(&second_neighbor).unwrap();
        database.add_node(&target).unwrap();
        database.add_node(&target_neighbor).unwrap();
        database.add_neighbor(this_node.public_key(), first_neighbor.public_key()).unwrap();
        database.add_neighbor(this_node.public_key(), target_neighbor.public_key()).unwrap();
        database.add_neighbor(first_neighbor.public_key(), this_node.public_key()).unwrap();
        database.add_neighbor(first_neighbor.public_key(), target_neighbor.public_key()).unwrap();
        database.add_neighbor(this_node.public_key(), second_neighbor.public_key()).unwrap();
        database.add_neighbor(second_neighbor.public_key(), this_node.public_key()).unwrap();
        database.add_neighbor(first_neighbor.public_key(), second_neighbor.public_key()).unwrap();
        database.add_neighbor(second_neighbor.public_key(), first_neighbor.public_key ()).unwrap ();
        database.add_neighbor (this_node.public_key (), target.public_key ()).unwrap ();
        database.add_neighbor (target.public_key (), this_node.public_key ()).unwrap ();
        database.add_neighbor (target.public_key (), target_neighbor.public_key ()).unwrap ();

        let subject = GossipProducerReal::new();

        let result = subject.produce(&database, target.public_key ());

        assert_contains (&result.node_records, &GossipNodeRecord::from(&this_node, true));
        assert_contains (&result.node_records, &GossipNodeRecord::from(&first_neighbor, false)); // this is the introduction because first_neighbor has fewer connections than second_neighbor
        assert_contains (&result.node_records, &GossipNodeRecord::from(&second_neighbor, true));
        assert_contains (&result.node_records, &GossipNodeRecord::from(&target, false));
        assert_contains (&result.node_records, &GossipNodeRecord::from(&target_neighbor, true));
        assert_eq!(result.node_records.len(), 5);
    }

    #[test]
    fn gossip_producer_does_not_introduce_target_to_bootstrap_nodes() {
        let mut this_node = make_node_record(1234, true, false);
        let mut first_neighbor = make_node_record(2345, true, false);
        let mut second_neighbor = make_node_record(3456, true, true);
        let mut target = make_node_record (4567, true, false);
        this_node.neighbors_mut().push (first_neighbor.public_key().clone ());
        this_node.neighbors_mut().push (second_neighbor.public_key().clone ());
        this_node.neighbors_mut().push (target.public_key().clone ());
        first_neighbor.neighbors_mut().push (this_node.public_key().clone ());
        first_neighbor.neighbors_mut().push (second_neighbor.public_key().clone ());
        second_neighbor.neighbors_mut().push (first_neighbor.public_key().clone ());
        second_neighbor.neighbors_mut().push (this_node.public_key().clone ());
        target.neighbors_mut().push (this_node.public_key().clone ());
        let mut database = NeighborhoodDatabase::new(this_node.public_key(),
                                                     this_node.node_addr_opt().as_ref().unwrap(), this_node.is_bootstrap_node(), &CryptDENull::from(this_node.public_key()));
        database.add_node(&first_neighbor).unwrap();
        database.add_node(&second_neighbor).unwrap();
        database.add_node(&target).unwrap();
        database.add_neighbor(this_node.public_key(), first_neighbor.public_key()).unwrap();
        database.add_neighbor(first_neighbor.public_key(), this_node.public_key()).unwrap();
        database.add_neighbor(this_node.public_key(), second_neighbor.public_key()).unwrap();
        database.add_neighbor(second_neighbor.public_key(), this_node.public_key()).unwrap();
        database.add_neighbor(first_neighbor.public_key(), second_neighbor.public_key()).unwrap();
        database.add_neighbor(second_neighbor.public_key(), first_neighbor.public_key ()).unwrap ();
        database.add_neighbor (this_node.public_key (), target.public_key ()).unwrap ();
        database.add_neighbor (target.public_key (), this_node.public_key ()).unwrap ();

        let subject = GossipProducerReal::new();

        let result = subject.produce(&database, target.public_key ());

        assert_contains (&result.node_records, &GossipNodeRecord::from(&this_node, true));
        assert_contains (&result.node_records, &GossipNodeRecord::from(&first_neighbor, true));
        assert_contains (&result.node_records, &GossipNodeRecord::from(&second_neighbor, false));
        assert_contains (&result.node_records, &GossipNodeRecord::from(&target, false));
        assert_eq!(result.node_records.len(), 4);
    }

    #[test]
    fn gossip_producer_does_not_introduce_target_to_more_nodes_than_it_needs() {
        let mut this_node = make_node_record(1234, true, false);
        let mut first_neighbor = make_node_record(2345, true, false);
        let mut second_neighbor = make_node_record(3456, true, false);
        let mut target = make_node_record (4567, true, false);
        let target_neighbor = make_node_record (5678, true, false);
        this_node.neighbors_mut().push (first_neighbor.public_key().clone ());
        this_node.neighbors_mut().push (second_neighbor.public_key().clone ());
        this_node.neighbors_mut().push (target_neighbor.public_key().clone ());
        this_node.neighbors_mut().push (target.public_key().clone ());
        first_neighbor.neighbors_mut().push (this_node.public_key().clone ());
        first_neighbor.neighbors_mut().push (second_neighbor.public_key().clone ());
        second_neighbor.neighbors_mut().push (first_neighbor.public_key().clone ());
        second_neighbor.neighbors_mut().push (this_node.public_key().clone ());
        target.neighbors_mut().push (this_node.public_key().clone ());
        target.neighbors_mut().push (target_neighbor.public_key().clone ());
        let mut database = NeighborhoodDatabase::new(this_node.public_key(),
                                                     this_node.node_addr_opt().as_ref().unwrap(), this_node.is_bootstrap_node(), &CryptDENull::from(this_node.public_key()));
        database.add_node(&first_neighbor).unwrap();
        database.add_node(&second_neighbor).unwrap();
        database.add_node(&target).unwrap();
        database.add_node(&target_neighbor).unwrap();
        database.add_neighbor(this_node.public_key(), first_neighbor.public_key()).unwrap();
        database.add_neighbor(first_neighbor.public_key(), this_node.public_key()).unwrap();
        database.add_neighbor(this_node.public_key(), second_neighbor.public_key()).unwrap();
        database.add_neighbor(second_neighbor.public_key(), this_node.public_key()).unwrap();
        database.add_neighbor(this_node.public_key(), target_neighbor.public_key()).unwrap();
        database.add_neighbor(first_neighbor.public_key(), second_neighbor.public_key()).unwrap();
        database.add_neighbor(second_neighbor.public_key(), first_neighbor.public_key ()).unwrap ();
        database.add_neighbor (this_node.public_key (), target.public_key ()).unwrap ();
        database.add_neighbor (target.public_key (), this_node.public_key ()).unwrap ();
        database.add_neighbor (target.public_key (), target_neighbor.public_key ()).unwrap ();

        let subject = GossipProducerReal::new();

        let result = subject.produce(&database, target.public_key ());

        assert_contains (&result.node_records, &GossipNodeRecord::from(&this_node, true));
        assert_contains (&result.node_records, &GossipNodeRecord::from(&target, false));
        assert_contains (&result.node_records, &GossipNodeRecord::from(&target_neighbor, true));

        // first_neighbor and second_neighbor have the same number of connections, so choosing which to introduce is non-deterministic
        let first_neighbor_gossip = result.node_records.iter().filter(|gnr| gnr.inner.public_key == *first_neighbor.public_key()).next().unwrap();
        let second_neighbor_gossip = result.node_records.iter().filter(|gnr| gnr.inner.public_key == *second_neighbor.public_key()).next().unwrap();
        assert_ne!(first_neighbor_gossip.inner.node_addr_opt.is_some(), second_neighbor_gossip.inner.node_addr_opt.is_some(), "exactly one neighbor should be introduced (both or neither actually were)");

        assert_eq!(result.node_records.len(), 5);
    }

    // TODO test about assuming that unknown target neighbors are not bootstrap when deciding how many introductions to make
    // ^^^ (not possible to set up yet because we can't add_neighbor a key for target that we don't already have in the DB as a NodeRecord)
    // This test will drive out the unimplemented!() in choose_introducees
}