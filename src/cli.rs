use std::path::PathBuf;

#[derive(StructOpt, Debug, Clone)]
#[structopt(name = "tempget", about = "Downloads files based on a template")]
pub struct CliOptions {
    #[structopt(default_value = "template.toml", parse(from_os_str))]
    /// The template file to use. By default, this is set to "template.toml".
    pub template_file: PathBuf,
    #[structopt(long = "no-extract")]
    /// When this flag is present, files are not extracted from the given zip
    /// files.
    pub no_extract: bool
}
