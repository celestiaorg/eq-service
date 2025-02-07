use std::sync::Arc;

use log::{debug, error, info, warn};
use tonic::{Request, Response, Status};

use eq_common::eqs::inclusion_server::Inclusion;
use eq_common::eqs::{
    get_keccak_inclusion_response::{ResponseValue, Status as ResponseStatus},
    GetKeccakInclusionRequest, GetKeccakInclusionResponse,
};

use celestia_types::{blob::Commitment, nmt::Namespace};

use crate::{InclusionService, Job, JobStatus};

// I hate this workaround. Kill it with fire.
pub struct InclusionServiceArc(pub Arc<InclusionService>);

#[tonic::async_trait]
impl Inclusion for InclusionServiceArc {
    async fn get_keccak_inclusion(
        &self,
        request: Request<GetKeccakInclusionRequest>,
    ) -> Result<Response<GetKeccakInclusionResponse>, Status> {
        let request = request.into_inner();
        let job = Job::new(
            request
                .height
                .try_into()
                .map_err(|_| Status::invalid_argument("Block Height must be u64"))?,
            // TODO: should we have some handling of versions here?
            Namespace::new_v0(&request.namespace).map_err(|_| {
                Status::invalid_argument("Namespace v0 expected! Must be 32 bytes, check encoding")
            })?,
            Commitment::new(request.commitment.try_into().map_err(|_| {
                Status::invalid_argument("Commitment must be 32 bytes, check encoding")
            })?),
        );

        info!("Received grpc request for: {job:?}");

        let job_key = bincode::serialize(&job).map_err(|e| Status::internal(e.to_string()))?;

        // Check DB for finished jobs
        if let Some(proof_data) = self
            .0
            .finished_db
            .get(&job_key)
            .map_err(|e| Status::internal(e.to_string()))?
        {
            let job_status: JobStatus =
                bincode::deserialize(&proof_data).map_err(|e| Status::internal(e.to_string()))?;
            match job_status {
                JobStatus::ZkProofFinished(proof) => {
                    debug!("Job finished, returning proof");
                    return Ok(Response::new(GetKeccakInclusionResponse {
                        status: ResponseStatus::ZkpFinished as i32,
                        response_value: Some(ResponseValue::Proof(
                            bincode::serialize(&proof)
                                .map_err(|e| Status::internal(e.to_string()))?,
                        )),
                    }));
                }
                JobStatus::Failed(error, maybe_status) => {
                    match maybe_status {
                        None => {
                            warn!("Job is PERMANENT FAILURE, returning status");
                            return Ok(Response::new(GetKeccakInclusionResponse {
                                status: ResponseStatus::PermanentFailure as i32,
                                response_value: Some(ResponseValue::ErrorMessage(format!(
                                    "{error:?}"
                                ))),
                            }));
                        }
                        Some(retry_status) => {
                            warn!("Job is Retryable Failure, returning status & retrying");
                            // We retry errors on each call to the gRPC
                            // for a specific [Job] by seding to the queue
                            match self.0.send_job_with_new_status(job_key, *retry_status, job) {
                                Ok(_) => {
                                    return Ok(Response::new(GetKeccakInclusionResponse {
                                        status: ResponseStatus::RetryableFailure as i32,
                                        response_value: Some(ResponseValue::ErrorMessage(format!(
                                            "Retrying! Previous error: {error:?}"
                                        ))),
                                    }));
                                }
                                Err(e) => {
                                    return Ok(Response::new(GetKeccakInclusionResponse {
                                        status: ResponseStatus::PermanentFailure as i32,
                                        response_value: Some(ResponseValue::ErrorMessage(format!(
                                            "Internal Failure: {e:?}"
                                        ))),
                                    }));
                                }
                            }
                        }
                    }
                }
                _ => {
                    let e = "Finished DB is in invalid state";
                    error!("{e}");
                    return Err(Status::internal(e));
                }
            }
        }

        // Check DB for pending jobs
        if let Some(queue_data) = self
            .0
            .queue_db
            .get(&job_key)
            .map_err(|e| Status::internal(e.to_string()))?
        {
            debug!("Job in pending queue");
            let job_status: JobStatus =
                bincode::deserialize(&queue_data).map_err(|e| Status::internal(e.to_string()))?;
            match job_status {
                JobStatus::DataAvailabilityPending => {
                    return Ok(Response::new(GetKeccakInclusionResponse {
                        status: ResponseStatus::DaPending as i32,
                        response_value: Some(ResponseValue::StatusMessage(
                            "Trying to collect DA inclusion proof".to_string(),
                        )),
                    }));
                }
                JobStatus::DataAvailable(_) => {
                    return Ok(Response::new(GetKeccakInclusionResponse {
                        status: ResponseStatus::DaAvailable as i32,
                        response_value: Some(ResponseValue::StatusMessage(
                            "Valid DA inclusion proof, requesting ZKP".to_string(),
                        )),
                    }));
                }
                JobStatus::ZkProofPending(job_id) => {
                    return Ok(Response::new(GetKeccakInclusionResponse {
                        status: ResponseStatus::ZkpPending as i32,
                        response_value: Some(ResponseValue::ProofId(job_id.to_vec())),
                    }));
                }
                _ => {
                    let e = "Job queue is in invalid state for {job:?}";
                    error!("{e}");
                    return Err(Status::internal(e));
                }
            }
        }

        info!("New {job:?} sending to worker and adding to queue");
        self.0
            .queue_db
            .insert(
                &job_key,
                bincode::serialize(&JobStatus::DataAvailabilityPending)
                    .map_err(|e| Status::internal(e.to_string()))?,
            )
            .map_err(|e| Status::internal(e.to_string()))?;

        self.0
            .job_sender
            .send(job.clone())
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(GetKeccakInclusionResponse {
            status: ResponseStatus::DaPending as i32,
            response_value: Some(ResponseValue::StatusMessage(
                "New job started! Call again for status and results".to_string(),
            )),
        }))
    }
}
