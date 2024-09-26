use clap::Parser;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    #[arg(
        short,
        long,
        default_value = "info",
        help = "Sets the log level of the varnish ingress controller"
    )]
    pub log_level: String,

    #[arg(
        long,
        default_value = "/etc/varnish/default.vcl",
        env = "VARNISH_VCL",
        help = "Sets the path to Varnish's default vcl file (the equivalent of Varnish's [-f] param)"
    )]
    pub vcl_file: String,

    #[arg(
        long,
        default_value = "./template/vcl.hbs",
        help = "Sets the path to the template file used to generate the VCL"
    )]
    pub template: String,

    #[arg(
        long,
        default_value = "varnish",
        help = "Sets the ingress class that controller will be looking for"
    )]
    pub ingress_class: String,

    #[arg(
        long,
        default_value = "/etc/varnish",
        env = "VARNISH_WORK_FOLDER",
        help = "Sets the working folder for the running Varnish instance\
             (the equivalent of Varnish's [-n] param)"
    )]
    pub work_folder: String,

    #[arg(
        long,
        default_value = "",
        env = "VARNISH_PARAMS",
        help = "Extra parameters sent to Varnish (the equivalent of Varnish's [-p] param)"
    )]
    pub params: String,

    #[arg(
        long,
        default_value = "",
        env = "VARNISH_STORAGE",
        help = "Storage backend for Varnish (the equivalent of Varnish's [-s] param)"
    )]
    pub storage: String,

    #[arg(
        long,
        default_value = "6081",
        env = "VARNISH_HTTP_PORT",
        help = "The http port at which Varnish will run"
    )]
    pub http_port: String,

    #[arg(
        long,
        env = "VARNISH_DEFAULT_TTL",
        default_value = "120s",
        help = "Default TTL for cached objects (the equivalent of Varnish's [-t] param)"
    )]
    pub default_ttl: String,

    #[arg(
        long,
        env = "VARNISH_VCL_SNIPPET",
        default_value = "",
        help = "Extra VCL code to be added at the end of the generated VCL"
    )]
    pub vcl_snippet: String,

    #[arg(
        long,
        env = "VARNISH_VCL_RECV_SNIPPET",
        default_value = "",
        help = "VCL code to be appended in the [vcl_recv] subroutine"
    )]
    pub vcl_recv_snippet: String,

    #[arg(
        long,
        env = "NAMESPACE",
        default_value = "default",
        help = "The namespace where Varnish Ingress Controller operates in"
    )]
    pub namespace: String,
}
