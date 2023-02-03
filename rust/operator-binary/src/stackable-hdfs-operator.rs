use clap::Parser;
use stackable_hdfs_crd::constants::APP_NAME;
use stackable_hdfs_crd::HdfsCluster;
use stackable_hdfs_crd::{v1, v2};
use stackable_hdfs_operator::OPERATOR_NAME;
use stackable_operator::k8s_openapi::ByteString;
use stackable_operator::kube::core::crd::merge_crds;
use stackable_operator::{
    cli::{Command, ProductOperatorRun},
    client, CustomResourceExt,
};
use tokio::sync::futures;

mod built_info {
    // The file has been placed there by the build script.
    include!(concat!(env!("OUT_DIR"), "/built.rs"));
}

#[derive(clap::Parser)]
#[clap(about = built_info::PKG_DESCRIPTION, author = stackable_operator::cli::AUTHOR)]
struct Opts {
    #[clap(subcommand)]
    cmd: Command,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let opts = Opts::parse();
    match opts.cmd {
        Command::Crd => print_crd_schema(),
        Command::Run(ProductOperatorRun {
            product_config,
            watch_namespace,
            tracing_target,
        }) => {
            stackable_operator::logging::initialize_logging(
                "HDFS_OPERATOR_LOG",
                APP_NAME,
                tracing_target,
            );

            stackable_operator::utils::print_startup_string(
                built_info::PKG_DESCRIPTION,
                built_info::PKG_VERSION,
                built_info::GIT_VERSION,
                built_info::TARGET,
                built_info::BUILT_TIME_UTC,
                built_info::RUSTC_VERSION,
            );
            let product_config = product_config.load(&[
                "deploy/config-spec/properties.yaml",
                "/etc/stackable/hdfs-operator/config-spec/properties.yaml",
            ])?;
            let client = client::create_client(Some(OPERATOR_NAME.to_string())).await?;

            let hdfs_controller =
                stackable_hdfs_operator::create_controller(client, product_config, watch_namespace);
            let conversion_webhook = stackable_hdfs_operator::webhook::start(&client);

            pin_mut!(hdfs_controller, conversion_webhook);

            futures::future::select(hdfs_controller, conversion_webhook).await;
        }
    };

    Ok(())
}

fn print_crd_schema() {
    use serde::{Deserialize, Serialize};
    use stackable_hdfs_crd;
    use stackable_operator::k8s_openapi::apiextensions_apiserver::pkg::apis::apiextensions::v1::{
        CustomResourceConversion, ServiceReference, WebhookClientConfig, WebhookConversion,
    };
    use stackable_operator::kube::CustomResourceExt;

    let crd_v1 = v1::HdfsCluster::crd();
    let crd_v2 = v2::HdfsCluster::crd();

    let crd_all_versions = vec![crd_v1.clone(), crd_v2.clone()];

    // apply schema where v1 is the stored version
    let mut bla = merge_crds(crd_all_versions.clone(), "v1alpha1").unwrap();

    bla.spec.conversion = Some(CustomResourceConversion {
        strategy: "Webhook".to_string(),
        webhook: Some(WebhookConversion {
            client_config: Some(WebhookClientConfig {
                ca_bundle: None,
                service: Some(ServiceReference {
                    name: "hdfs-operator".to_string(),
                    namespace: "default".to_string(),
                    path: Some("/convert".to_string()),
                    port: Some(1234),
                }),
                url: None,
            }),
            conversion_review_versions: vec!["v1".to_string()],
        }),
    });

    println!("{}", serde_yaml::to_string(&bla).unwrap());
}
