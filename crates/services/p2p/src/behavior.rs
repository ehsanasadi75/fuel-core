use std::sync::{
    Arc,
    RwLock,
};

use crate::{
    codecs::NetworkCodec,
    config::Config,
    discovery::{
        DiscoveryBehaviour,
        DiscoveryConfig,
        DiscoveryEvent,
    },
    gossipsub::{
        config::build_gossipsub_behaviour,
        topics::GossipTopic,
    },
    peer_manager::{
        ConnectionState,
        PeerInfo,
        PeerInfoEvent,
        PeerManagerBehaviour,
    },
    request_response::messages::{
        NetworkResponse,
        RequestMessage,
    },
};
use fuel_core_types::blockchain::primitives::BlockHeight;
use libp2p::{
    gossipsub::{
        error::{
            PublishError,
            SubscriptionError,
        },
        Gossipsub,
        GossipsubEvent,
        MessageAcceptance,
        MessageId,
    },
    request_response::{
        ProtocolSupport,
        RequestId,
        RequestResponse,
        RequestResponseConfig,
        RequestResponseEvent,
        ResponseChannel,
    },
    swarm::NetworkBehaviour,
    Multiaddr,
    PeerId,
};

#[derive(Debug)]
pub enum FuelBehaviourEvent {
    Discovery(DiscoveryEvent),
    PeerInfo(PeerInfoEvent),
    Gossipsub(GossipsubEvent),
    RequestResponse(RequestResponseEvent<RequestMessage, NetworkResponse>),
}

/// Handles all p2p protocols needed for Fuel.
#[derive(NetworkBehaviour)]
#[behaviour(out_event = "FuelBehaviourEvent")]
pub struct FuelBehaviour<Codec: NetworkCodec> {
    /// Node discovery
    discovery: DiscoveryBehaviour,

    /// Handles Peer Connections
    /// Identifies and periodically requests `BlockHeight` from connected nodes
    peer_manager: PeerManagerBehaviour,

    /// Message propagation for p2p
    gossipsub: Gossipsub,

    /// RequestResponse protocol
    request_response: RequestResponse<Codec>,
}

impl<Codec: NetworkCodec> FuelBehaviour<Codec> {
    pub(crate) fn new(
        p2p_config: &Config,
        codec: Codec,
        connection_state: Arc<RwLock<ConnectionState>>,
    ) -> Self {
        let local_public_key = p2p_config.keypair.public();
        let local_peer_id = PeerId::from_public_key(&local_public_key);

        let discovery_config = {
            let mut discovery_config =
                DiscoveryConfig::new(local_peer_id, p2p_config.network_name.clone());

            discovery_config
                .enable_mdns(p2p_config.enable_mdns)
                .discovery_limit(p2p_config.max_peers_connected as usize)
                .allow_private_addresses(p2p_config.allow_private_addresses)
                .with_bootstrap_nodes(p2p_config.bootstrap_nodes.clone())
                .with_reserved_nodes(p2p_config.reserved_nodes.clone())
                .enable_reserved_nodes_only_mode(p2p_config.reserved_nodes_only_mode);

            if let Some(random_walk) = p2p_config.random_walk {
                discovery_config.with_random_walk(random_walk);
            }

            if let Some(duration) = p2p_config.connection_idle_timeout {
                discovery_config.set_connection_idle_timeout(duration);
            }

            discovery_config
        };

        let peer_manager = PeerManagerBehaviour::new(p2p_config, connection_state);

        let req_res_protocol =
            std::iter::once((codec.get_req_res_protocol(), ProtocolSupport::Full));

        let mut req_res_config = RequestResponseConfig::default();
        req_res_config.set_request_timeout(p2p_config.set_request_timeout);
        req_res_config.set_connection_keep_alive(p2p_config.set_connection_keep_alive);

        let request_response =
            RequestResponse::new(codec, req_res_protocol, req_res_config);

        Self {
            discovery: discovery_config.finish(),
            gossipsub: build_gossipsub_behaviour(p2p_config),
            peer_manager,
            request_response,
        }
    }

    pub fn add_addresses_to_peer_info(
        &mut self,
        peer_id: &PeerId,
        addresses: Vec<Multiaddr>,
    ) {
        self.peer_manager.insert_peer_addresses(peer_id, addresses);
    }

    pub fn add_addresses_to_discovery(
        &mut self,
        peer_id: &PeerId,
        addresses: Vec<Multiaddr>,
    ) {
        for address in addresses {
            self.discovery.add_address(peer_id, address.clone());
        }
    }

    pub fn get_peers_ids(&self) -> impl Iterator<Item = &PeerId> {
        self.peer_manager.get_peers_ids()
    }

    pub fn total_peers_connected(&self) -> usize {
        self.peer_manager.total_peers_connected()
    }

    pub fn publish_message(
        &mut self,
        topic: GossipTopic,
        encoded_data: Vec<u8>,
    ) -> Result<MessageId, PublishError> {
        self.gossipsub.publish(topic, encoded_data)
    }

    pub fn subscribe_to_topic(
        &mut self,
        topic: &GossipTopic,
    ) -> Result<bool, SubscriptionError> {
        self.gossipsub.subscribe(topic)
    }

    pub fn send_request_msg(
        &mut self,
        message_request: RequestMessage,
        peer_id: &PeerId,
    ) -> RequestId {
        self.request_response.send_request(peer_id, message_request)
    }

    pub fn send_response_msg(
        &mut self,
        channel: ResponseChannel<NetworkResponse>,
        message: NetworkResponse,
    ) -> Result<(), NetworkResponse> {
        self.request_response.send_response(channel, message)
    }

    pub fn report_message_validation_result(
        &mut self,
        msg_id: &MessageId,
        propagation_source: &PeerId,
        acceptance: MessageAcceptance,
    ) -> Result<bool, PublishError> {
        self.gossipsub.report_message_validation_result(
            msg_id,
            propagation_source,
            acceptance,
        )
    }

    pub fn update_block_height(&mut self, block_height: BlockHeight) {
        self.peer_manager.update_block_height(block_height);
    }

    // Currently only used in testing, but should be useful for the P2P Service API
    #[allow(dead_code)]
    pub fn get_peer_info(&self, peer_id: &PeerId) -> Option<&PeerInfo> {
        self.peer_manager.get_peer_info(peer_id)
    }

    pub fn peer_manager(&self) -> &PeerManagerBehaviour {
        &self.peer_manager
    }
}

impl From<DiscoveryEvent> for FuelBehaviourEvent {
    fn from(event: DiscoveryEvent) -> Self {
        FuelBehaviourEvent::Discovery(event)
    }
}

impl From<PeerInfoEvent> for FuelBehaviourEvent {
    fn from(event: PeerInfoEvent) -> Self {
        FuelBehaviourEvent::PeerInfo(event)
    }
}

impl From<GossipsubEvent> for FuelBehaviourEvent {
    fn from(event: GossipsubEvent) -> Self {
        FuelBehaviourEvent::Gossipsub(event)
    }
}

impl From<RequestResponseEvent<RequestMessage, NetworkResponse>> for FuelBehaviourEvent {
    fn from(event: RequestResponseEvent<RequestMessage, NetworkResponse>) -> Self {
        FuelBehaviourEvent::RequestResponse(event)
    }
}
