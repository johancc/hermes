pub mod fitness_v1_types;
use async_google_apis_common as common;
use async_trait::async_trait;
use chrono;
use common::yup_oauth2::{InstalledFlowAuthenticator, InstalledFlowReturnMethod};
use hyper::client::HttpConnector;
use hyper_rustls::HttpsConnector;
use std::sync::Arc;

use crate::providers::google::fitness_v1_types::{
    AggregateBy, AggregateResponse, BucketByTime, UsersDatasetAggregateParams,
};

use self::fitness_v1_types::AggregateRequest;

use super::Provider;

const GOOGLE_FIT_DATA_SOURCE_ID: &'static str =
    "derived:com.google.step_count.delta:com.google.android.gms:estimated_steps";
const GOOGLE_FIT_STEPS_DATATYPE_NAME: &'static str = "com.google.step_count.delta";

/// GoogleFitProvider - Interfaces with the Google Fitness API
/// to retrieve daily steps and heart points.
pub struct GoogleFitProvider {
    user_dataset_service: fitness_v1_types::UsersDatasetService,
}

impl GoogleFitProvider {
    /// Creates a new Provider which talks to the Google Fitness API to retrieve the user's
    /// daily step count. Looks for the Google Client credentials using `GOOGLE_CLIENT_SECRET`
    /// if `client_secret_path` is not provided.
    /// Will launch an authentication flow process for the user to give the program the necessary permissions.
    pub async fn new(client_secret_path: Option<String>) -> Result<GoogleFitProvider, ()> {
        let client_secret_path = client_secret_path
            .unwrap_or_else(|| std::env::var("GOOGLE_CLIENT_SECRET").unwrap().to_string());
        GoogleFitProvider::validate(client_secret_path.clone());

        let https_client = GoogleFitProvider::generate_https_client();
        let auth = GoogleFitProvider::generate_auth(https_client.clone(), client_secret_path).await;

        let mut user_dataset_service =
            fitness_v1_types::UsersDatasetService::new(https_client, Arc::new(auth.clone()));
        let scopes = vec![fitness_v1_types::FitnessScopes::FitnessActivityRead];
        user_dataset_service.set_scopes(scopes);

        Ok(GoogleFitProvider {
            user_dataset_service,
        })
    }

    async fn generate_auth(
        https_client: common::TlsClient,
        client_secret_path: String,
    ) -> yup_oauth2::authenticator::Authenticator<HttpsConnector<HttpConnector>> {
        
        let secrets = common::yup_oauth2::read_application_secret(client_secret_path)
            .await
            .expect("client secret file is invalid");

        let auth =
            InstalledFlowAuthenticator::builder(secrets, InstalledFlowReturnMethod::HTTPRedirect)
                .persist_tokens_to_disk("tmp_client_token.json")
                .hyper_client(https_client)
                .build()
                .await
                .expect("Failed to authenticate");
        auth
    }

    fn generate_https_client() -> common::TlsClient {
        let conn = hyper_rustls::HttpsConnector::with_native_roots();
        let cl = hyper::Client::builder().build(conn);
        cl
    }

    fn validate(client_secret_path: String) {
        if !GoogleFitProvider::check_client_secret(client_secret_path) {
            panic!("Invalid client secret path");
        };
    }

    fn check_client_secret(client_secret_path: String) -> bool {
        std::path::Path::new(&client_secret_path).exists()
    }
}

#[async_trait]
impl Provider for GoogleFitProvider {
    async fn daily_steps(&self) -> anyhow::Result<i32> {
        let request = self.generate_request();
        let params = self.generate_params();
        let resp = self
            .user_dataset_service
            .aggregate(&params, &request)
            .await?;

        let steps = self.get_step_count_from_resp(resp);

        steps
    }
}

impl GoogleFitProvider {
    /// Creates an UserDatasetAggregateParams that requests data about the current user.
    fn generate_params(&self) -> UsersDatasetAggregateParams {
        UsersDatasetAggregateParams {
            user_id: "me".to_string(),
            fitness_params: None,
        }
    }

    /// Creates an AggregateRequest which requests all the steps between the current time
    /// and the start of the current day (based on the current timezone).
    fn generate_request(&self) -> AggregateRequest {
        let (midnight, current, delta) = self.generate_timestamps_now_and_midnight();
        let req = AggregateRequest {
            aggregate_by: Some(vec![AggregateBy {
                data_source_id: Some(String::from(GOOGLE_FIT_DATA_SOURCE_ID)),
                data_type_name: Some(String::from(GOOGLE_FIT_STEPS_DATATYPE_NAME)),
            }]),
            bucket_by_time: Some(BucketByTime {
                duration_millis: Some(delta),
                ..BucketByTime::default()
            }),
            start_time_millis: Some(midnight),
            end_time_millis: Some(current),
            ..AggregateRequest::default()
        };

        req
    }

    /// Creates two UNIX timestamps: `(midnight, current, delta)`.
    /// `midnight` is the UNIX timestamp from midnight (where "midnight" is relative to the local timezone).
    /// `current` is the current UNIX timestamp.
    /// `delta` is the number of milliseconds between the two timestamps.
    fn generate_timestamps_now_and_midnight(&self) -> (String, String, String) {
        let current_time = chrono::offset::Utc::now();
        let midnight_time = chrono::offset::Local::today().and_hms_milli(0, 0, 0, 0);

        let current_time_utc = current_time.timestamp_millis();
        let midnight_utc = midnight_time.timestamp_millis();
        let delta = current_time_utc - midnight_utc;
        (
            midnight_utc.to_string(),
            current_time_utc.to_string(),
            delta.to_string(),
        )
    }

    /// Extracts the steps from AggregateResponse. Assumes the appropriate AggregateRequest was sent.
    fn get_step_count_from_resp(&self, resp: AggregateResponse) -> anyhow::Result<i32> {
        let steps = resp
            .bucket
            .unwrap()
            .iter()
            .flat_map(|aggregate_bucket| {
                aggregate_bucket
                    .dataset
                    .as_ref()
                    .expect("AggregateBucket has no datapoints")
            })
            .flat_map(|dataset| dataset.point.as_ref().expect("Invalid dataset"))
            .flat_map(|point| point.value.as_ref().expect("Empty datapoint").iter())
            .map(|val| val.int_val.expect("Invalid data value"))
            .collect::<Vec<i32>>()
            .into_iter()
            .sum::<i32>();

        anyhow::Result::Ok(steps)
    }
}
