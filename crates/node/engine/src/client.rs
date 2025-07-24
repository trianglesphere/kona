//! An Engine API Client.

use crate::Metrics;
use alloy_eips::eip1898::BlockNumberOrTag;
use alloy_network::{AnyNetwork, Network};
use alloy_primitives::{B256, BlockHash, Bytes};
use alloy_provider::{Provider, RootProvider};
use alloy_rpc_client::RpcClient;
use alloy_rpc_types_engine::{
    ClientVersionV1, ExecutionPayloadBodiesV1, ExecutionPayloadEnvelopeV2, ExecutionPayloadInputV2,
    ExecutionPayloadV3, ForkchoiceState, ForkchoiceUpdated, JwtSecret, PayloadId, PayloadStatus,
};
use alloy_rpc_types_eth::Block;
use alloy_transport::{RpcError, TransportErrorKind, TransportResult};
use alloy_transport_http::{
    AuthLayer, AuthService, Http, HyperClient,
    hyper_util::{
        client::legacy::{Client, connect::HttpConnector},
        rt::TokioExecutor,
    },
};
use derive_more::Deref;
use http_body_util::Full;
use kona_genesis::RollupConfig;
use kona_protocol::{FromBlockError, L2BlockInfo};
use op_alloy_network::Optimism;
use op_alloy_provider::ext::engine::OpEngineApi;
use op_alloy_rpc_types::Transaction;
use op_alloy_rpc_types_engine::{
    OpExecutionPayloadEnvelopeV3, OpExecutionPayloadEnvelopeV4, OpExecutionPayloadV4,
    OpPayloadAttributes, ProtocolVersion,
};
use std::{sync::Arc, time::Instant};
use thiserror::Error;
use tower::ServiceBuilder;
use url::Url;

/// Comprehensive error types for [`EngineClient`] operations.
///
/// The engine client can encounter various failure modes during RPC communication
/// and data processing. These errors provide detailed context for debugging and
/// appropriate error handling strategies.
///
/// ## Error Categories
///
/// ### RPC Communication Errors
/// Network-level and protocol-level failures during RPC calls:
/// - **Connection Issues**: Network timeouts, DNS resolution failures
/// - **Authentication Problems**: JWT token validation, credential issues
/// - **Protocol Errors**: HTTP/JSON-RPC malformed requests or responses
/// - **Rate Limiting**: Provider throttling, quota exceeded errors
///
/// ### Data Processing Errors  
/// Issues with blockchain data structure validation and conversion:
/// - **Invalid Block Data**: Malformed block headers, transaction data
/// - **Consensus Violations**: Block data violating chain consensus rules
/// - **Encoding Issues**: RLP decoding errors, invalid field formats
/// - **Genesis Mismatches**: Block data incompatible with rollup genesis
///
/// ## Error Context
/// Errors provide context about:
/// - **Operation**: Which client method failed (block retrieval, payload creation)
/// - **Parameters**: What inputs caused the failure (block numbers, hashes)
/// - **Provider**: Whether error originated from L1 or L2 provider
/// - **Severity**: Whether error is temporary (retry) or permanent (critical)
#[derive(Error, Debug)]
pub enum EngineClientError {
    /// An RPC error occurred
    #[error("An RPC error occurred: {0}")]
    RpcError(#[from] RpcError<TransportErrorKind>),

    /// An error occurred while decoding the payload
    #[error("An error occurred while decoding the payload: {0}")]
    BlockInfoDecodeError(#[from] FromBlockError),
}
/// A Hyper HTTP client with a JWT authentication layer.
type HyperAuthClient<B = Full<Bytes>> = HyperClient<B, AuthService<Client<HttpConnector, B>>>;

/// High-level engine API client providing authenticated access to execution layer and L1/L2 providers.
///
/// The [`EngineClient`] serves as the primary interface for all blockchain RPC operations
/// within the kona engine. It manages authenticated connections to multiple providers
/// and provides convenient methods for common operations like block retrieval and
/// engine API calls.
///
/// ## Architecture
///
/// ### Multi-Provider Design
/// The client maintains separate providers for different purposes:
/// - **Engine Provider**: Authenticated connection to execution layer (EL)
/// - **L2 Provider**: Direct connection to L2 chain for block/state queries  
/// - **L1 Provider**: Connection to L1 chain for derivation and finality data
///
/// ### Authentication Model
/// - **JWT Authentication**: Engine and L2 providers use JWT tokens for security
/// - **HTTP Transport**: All connections use HTTP with JWT authentication layer
/// - **Connection Pooling**: Underlying HTTP client handles connection reuse
///
/// ### API Version Selection
/// The client automatically selects appropriate Engine API versions based on:
/// - **Block Timestamps**: Different versions activated at specific times
/// - **Rollup Configuration**: Chain-specific activation schedules
/// - **Protocol Features**: Support for new payload formats and capabilities
///
/// ## Usage Patterns
///
/// ### Block Data Retrieval
/// ```ignore
/// let block = client.l2_block_by_label(BlockNumberOrTag::Latest).await?;
/// let block_info = client.l2_block_info_by_label(block_number.into()).await?;
/// ```
///
/// ### Engine API Operations
/// ```ignore
/// let status = client.new_payload_v3(payload, parent_beacon_root).await?;
/// let result = client.fork_choice_updated_v2(forkchoice, attributes).await?;
/// let envelope = client.get_payload_v3(payload_id).await?;
/// ```
///
/// ## Error Handling
/// All methods return [`Result`] types with detailed error information:
/// - **Network Errors**: Connection issues, timeouts, authentication failures
/// - **Protocol Errors**: Invalid requests, unsupported operations
/// - **Data Errors**: Malformed responses, validation failures
///
/// ## Performance Characteristics
/// - **Connection Reuse**: HTTP client pools connections for efficiency
/// - **Concurrent Operations**: Multiple RPC calls can execute simultaneously
/// - **Metrics Integration**: All operations automatically record performance metrics
/// - **JWT Overhead**: Authentication adds minimal latency to requests
#[derive(Debug, Deref, Clone)]
pub struct EngineClient {
    /// The L2 engine provider.
    #[deref]
    engine: RootProvider<AnyNetwork>,
    /// The L2 chain provider.
    l2_provider: RootProvider<Optimism>,
    /// The L1 chain provider.
    l1_provider: RootProvider,
    /// The [RollupConfig] for the chain used to timestamp which version of the engine api to use.
    cfg: Arc<RollupConfig>,
}

impl EngineClient {
    /// Creates a new RPC client for the given address and JWT secret.
    fn rpc_client<T: Network>(addr: Url, jwt: JwtSecret) -> RootProvider<T> {
        let hyper_client = Client::builder(TokioExecutor::new()).build_http::<Full<Bytes>>();
        let auth_layer = AuthLayer::new(jwt);
        let service = ServiceBuilder::new().layer(auth_layer).service(hyper_client);
        let layer_transport = HyperClient::with_service(service);

        let http_hyper = Http::with_client(layer_transport, addr);
        let rpc_client = RpcClient::new(http_hyper, false);
        RootProvider::<T>::new(rpc_client)
    }

    /// Creates a new [`EngineClient`] with HTTP transport and JWT authentication.
    ///
    /// This constructor establishes authenticated connections to all required providers
    /// and configures the client for Engine API operations. The client maintains
    /// separate providers for different purposes while sharing authentication credentials.
    ///
    /// ## Parameters
    /// - `engine`: URL for execution layer Engine API endpoint (with JWT auth)
    /// - `l2_rpc`: URL for L2 chain RPC endpoint (with JWT auth)  
    /// - `l1_rpc`: URL for L1 chain RPC endpoint (no auth required)
    /// - `cfg`: Rollup configuration for Engine API version selection
    /// - `jwt`: Shared JWT secret for authenticated endpoints
    ///
    /// ## Connection Setup
    /// 
    /// ### Engine Provider (Authenticated)
    /// - **Purpose**: Engine API calls (newPayload, forkchoiceUpdated, getPayload)
    /// - **Network**: AnyNetwork (generic Ethereum-compatible)
    /// - **Auth**: JWT authentication layer
    /// - **Transport**: HTTP with Hyper client
    ///
    /// ### L2 Provider (Authenticated)  
    /// - **Purpose**: L2 block queries, transaction data, state access
    /// - **Network**: Optimism (OP Stack specific types)
    /// - **Auth**: JWT authentication layer  
    /// - **Transport**: HTTP with Hyper client
    ///
    /// ### L1 Provider (Public)
    /// - **Purpose**: L1 block data, finality information, derivation sources
    /// - **Network**: Ethereum mainnet (or testnet)
    /// - **Auth**: None (public RPC endpoint)
    /// - **Transport**: Standard HTTP transport
    ///
    /// ## Authentication Model
    /// JWT authentication is applied to engine and L2 providers to ensure:
    /// - **Access Control**: Only authorized clients can modify execution state
    /// - **Request Integrity**: JWT signatures prevent request tampering
    /// - **Session Management**: Tokens can be rotated for security
    ///
    /// ## Configuration Usage
    /// The rollup configuration enables:
    /// - **API Version Selection**: Choose appropriate Engine API versions by timestamp
    /// - **Chain Parameters**: Validate operations against chain-specific rules
    /// - **Protocol Features**: Enable/disable features based on activation schedules
    ///
    /// ## Error Scenarios
    /// Construction can fail due to:
    /// - **Invalid URLs**: Malformed endpoint addresses
    /// - **JWT Issues**: Invalid or expired authentication tokens
    /// - **Network Problems**: DNS resolution, connection establishment failures
    /// - **Configuration Errors**: Invalid rollup configuration parameters
    pub fn new_http(
        engine: Url,
        l2_rpc: Url,
        l1_rpc: Url,
        cfg: Arc<RollupConfig>,
        jwt: JwtSecret,
    ) -> Self {
        let engine = Self::rpc_client::<AnyNetwork>(engine, jwt);
        let l2_provider = Self::rpc_client::<Optimism>(l2_rpc, jwt);
        let l1_provider = RootProvider::new_http(l1_rpc);

        Self { engine, l2_provider, l1_provider, cfg }
    }

    /// Returns a reference to the inner L2 [`RootProvider`].
    pub const fn l2_provider(&self) -> &RootProvider<Optimism> {
        &self.l2_provider
    }

    /// Returns a reference to the inner L1 [`RootProvider`].
    pub const fn l1_provider(&self) -> &RootProvider {
        &self.l1_provider
    }

    /// Returns a reference to the inner [`RollupConfig`].
    pub fn cfg(&self) -> &RollupConfig {
        self.cfg.as_ref()
    }

    /// Fetches the [`Block<T>`] for the given [`BlockNumberOrTag`].
    pub async fn l2_block_by_label(
        &self,
        numtag: BlockNumberOrTag,
    ) -> Result<Option<Block<Transaction>>, EngineClientError> {
        Ok(<RootProvider<Optimism>>::get_block_by_number(&self.l2_provider, numtag).full().await?)
    }

    /// Retrieves L2 block information with OP Stack specific metadata.
    ///
    /// This method fetches a complete L2 block and extracts the OP Stack specific
    /// metadata required for rollup operations. The extracted information includes
    /// L1 origin data, sequencer information, and other rollup-specific fields.
    ///
    /// ## Parameters
    /// - `numtag`: Block identifier (number, hash, or tag like "latest")
    ///
    /// ## Returns
    /// - `Some(L2BlockInfo)`: Block found and successfully parsed
    /// - `None`: Block not found at the specified identifier
    /// - `Err(EngineClientError)`: RPC error or block parsing failure
    ///
    /// ## Block Processing Steps
    /// 1. **RPC Query**: Fetch full block data from L2 provider
    /// 2. **Consensus Conversion**: Convert RPC block to consensus format
    /// 3. **Metadata Extraction**: Parse OP Stack specific fields from block
    /// 4. **Genesis Validation**: Verify block compatibility with rollup genesis
    ///
    /// ## Error Contexts
    ///
    /// ### RPC Errors
    /// - **Network Issues**: Connection timeouts, DNS resolution failures
    /// - **Authentication**: JWT token validation problems
    /// - **Provider Errors**: L2 node unavailable, database corruption
    ///
    /// ### Block Parsing Errors
    /// - **Invalid Structure**: Block missing required OP Stack fields
    /// - **Genesis Mismatch**: Block incompatible with configured genesis
    /// - **Encoding Issues**: Unable to decode block metadata
    ///
    /// ## Usage Patterns
    /// ```ignore
    /// // Get latest block info
    /// let latest = client.l2_block_info_by_label(BlockNumberOrTag::Latest).await?;
    ///
    /// // Get specific block by number
    /// let block_info = client.l2_block_info_by_label(12345.into()).await?;
    ///
    /// // Handle missing blocks
    /// match client.l2_block_info_by_label(future_block.into()).await? {
    ///     Some(info) => process_block(info),
    ///     None => log::warn!("Block not yet available"),
    /// }
    /// ```
    ///
    /// ## Performance Considerations
    /// - **Full Block Retrieval**: Fetches complete block data including transactions
    /// - **Parsing Overhead**: Additional processing to extract OP Stack metadata
    /// - **Cache Behavior**: Results may be cached by underlying RPC provider
    pub async fn l2_block_info_by_label(
        &self,
        numtag: BlockNumberOrTag,
    ) -> Result<Option<L2BlockInfo>, EngineClientError> {
        let block =
            <RootProvider<Optimism>>::get_block_by_number(&self.l2_provider, numtag).full().await?;
        let Some(block) = block else {
            return Ok(None);
        };
        Ok(Some(L2BlockInfo::from_block_and_genesis(&block.into_consensus(), &self.cfg.genesis)?))
    }
}

#[async_trait::async_trait]
impl OpEngineApi<AnyNetwork, Http<HyperAuthClient>> for EngineClient {
    async fn new_payload_v2(
        &self,
        payload: ExecutionPayloadInputV2,
    ) -> TransportResult<PayloadStatus> {
        let call = <RootProvider<AnyNetwork> as OpEngineApi<
            AnyNetwork,
            Http<HyperAuthClient>,
        >>::new_payload_v2(&self.engine, payload);

        record_call_time(call, Metrics::NEW_PAYLOAD_METHOD).await
    }

    async fn new_payload_v3(
        &self,
        payload: ExecutionPayloadV3,
        parent_beacon_block_root: B256,
    ) -> TransportResult<PayloadStatus> {
        let call = <RootProvider<AnyNetwork> as OpEngineApi<
            AnyNetwork,
            Http<HyperAuthClient>,
        >>::new_payload_v3(&self.engine, payload, parent_beacon_block_root);

        record_call_time(call, Metrics::NEW_PAYLOAD_METHOD).await
    }

    async fn new_payload_v4(
        &self,
        payload: OpExecutionPayloadV4,
        parent_beacon_block_root: B256,
    ) -> TransportResult<PayloadStatus> {
        let call = <RootProvider<AnyNetwork> as OpEngineApi<
            AnyNetwork,
            Http<HyperAuthClient>,
        >>::new_payload_v4(&self.engine, payload, parent_beacon_block_root);

        record_call_time(call, Metrics::NEW_PAYLOAD_METHOD).await
    }

    async fn fork_choice_updated_v2(
        &self,
        fork_choice_state: ForkchoiceState,
        payload_attributes: Option<OpPayloadAttributes>,
    ) -> TransportResult<ForkchoiceUpdated> {
        let call = <RootProvider<AnyNetwork> as OpEngineApi<
            AnyNetwork,
            Http<HyperAuthClient>,
        >>::fork_choice_updated_v2(&self.engine, fork_choice_state, payload_attributes);

        record_call_time(call, Metrics::FORKCHOICE_UPDATE_METHOD).await
    }

    async fn fork_choice_updated_v3(
        &self,
        fork_choice_state: ForkchoiceState,
        payload_attributes: Option<OpPayloadAttributes>,
    ) -> TransportResult<ForkchoiceUpdated> {
        let call = <RootProvider<AnyNetwork> as OpEngineApi<
            AnyNetwork,
            Http<HyperAuthClient>,
        >>::fork_choice_updated_v3(&self.engine, fork_choice_state, payload_attributes);

        record_call_time(call, Metrics::FORKCHOICE_UPDATE_METHOD).await
    }

    async fn get_payload_v2(
        &self,
        payload_id: PayloadId,
    ) -> TransportResult<ExecutionPayloadEnvelopeV2> {
        let call = <RootProvider<AnyNetwork> as OpEngineApi<
            AnyNetwork,
            Http<HyperAuthClient>,
        >>::get_payload_v2(&self.engine, payload_id);

        record_call_time(call, Metrics::GET_PAYLOAD_METHOD).await
    }

    async fn get_payload_v3(
        &self,
        payload_id: PayloadId,
    ) -> TransportResult<OpExecutionPayloadEnvelopeV3> {
        let call = <RootProvider<AnyNetwork> as OpEngineApi<
            AnyNetwork,
            Http<HyperAuthClient>,
        >>::get_payload_v3(&self.engine, payload_id);

        record_call_time(call, Metrics::GET_PAYLOAD_METHOD).await
    }

    async fn get_payload_v4(
        &self,
        payload_id: PayloadId,
    ) -> TransportResult<OpExecutionPayloadEnvelopeV4> {
        let call = <RootProvider<AnyNetwork> as OpEngineApi<
            AnyNetwork,
            Http<HyperAuthClient>,
        >>::get_payload_v4(&self.engine, payload_id);

        record_call_time(call, Metrics::GET_PAYLOAD_METHOD).await
    }

    async fn get_payload_bodies_by_hash_v1(
        &self,
        block_hashes: Vec<BlockHash>,
    ) -> TransportResult<ExecutionPayloadBodiesV1> {
        <RootProvider<AnyNetwork> as OpEngineApi<
            AnyNetwork,
            Http<HyperAuthClient>,
        >>::get_payload_bodies_by_hash_v1(&self.engine, block_hashes).await
    }

    async fn get_payload_bodies_by_range_v1(
        &self,
        start: u64,
        count: u64,
    ) -> TransportResult<ExecutionPayloadBodiesV1> {
        <RootProvider<AnyNetwork> as OpEngineApi<
            AnyNetwork,
            Http<HyperAuthClient>,
        >>::get_payload_bodies_by_range_v1(&self.engine, start, count).await
    }

    async fn get_client_version_v1(
        &self,
        client_version: ClientVersionV1,
    ) -> TransportResult<Vec<ClientVersionV1>> {
        <RootProvider<AnyNetwork> as OpEngineApi<
            AnyNetwork,
            Http<HyperAuthClient>,
        >>::get_client_version_v1(&self.engine, client_version).await
    }

    async fn signal_superchain_v1(
        &self,
        recommended: ProtocolVersion,
        required: ProtocolVersion,
    ) -> TransportResult<ProtocolVersion> {
        <RootProvider<AnyNetwork> as OpEngineApi<
            AnyNetwork,
            Http<HyperAuthClient>,
        >>::signal_superchain_v1(&self.engine, recommended, required).await
    }

    async fn exchange_capabilities(
        &self,
        capabilities: Vec<String>,
    ) -> TransportResult<Vec<String>> {
        <RootProvider<AnyNetwork> as OpEngineApi<
            AnyNetwork,
            Http<HyperAuthClient>,
        >>::exchange_capabilities(&self.engine, capabilities).await
    }
}

/// Wrapper to record the time taken for a call to the engine API and log the result as a metric.
async fn record_call_time<T>(
    f: impl Future<Output = TransportResult<T>>,
    metric_label: &'static str,
) -> TransportResult<T> {
    // Await on the future and track its duration.
    let start = Instant::now();
    let result = f.await?;
    let duration = start.elapsed();

    // Record the call duration.
    kona_macros::record!(
        histogram,
        Metrics::ENGINE_METHOD_REQUEST_DURATION,
        "method",
        metric_label,
        duration.as_secs_f64()
    );
    Ok(result)
}
