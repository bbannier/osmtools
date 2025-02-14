use std::{
    collections::{BTreeMap, HashMap},
    io::{self, stdout, BufWriter, Write},
    path::PathBuf,
};

use anyhow::Result;
use clap::{Parser, Subcommand};
use itertools::Itertools;
use log::info;
use osmpbfreader::{OsmId, OsmObj, OsmPbfReader};
use serde_json::to_string;
use simple_logger::SimpleLogger;

const TARGET_BOUNDARY_TYPES: &[&str] = &[
    "administrative",
    "state_border",
    "country_border",
    "state border",
];

#[derive(Parser)]
struct Cli {
    /// PBF file to read.
    #[arg(short, long)]
    in_file: PathBuf,

    /// Path to output file. If unspecified output is written to stdout.
    #[arg(short, long)]
    out_file: Option<PathBuf>,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    Stats,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    SimpleLogger::new()
        .with_level(log::LevelFilter::Info)
        .init()?;

    info!("Unpacking relations from {:?}", cli.in_file);

    let out: Box<dyn io::Write> = if let Some(f) = cli.out_file {
        let f = std::fs::File::create(f)?;
        Box::new(f)
    } else {
        Box::new(stdout())
    };

    if let Some(Commands::Stats) = cli.command {
        let relations = load_relations(cli.in_file, filter_target_relations)?;
        info!("Gathering some stats..");
        to_stats(&relations, out)?;
    } else {
        let relations = load_relations(cli.in_file, filter_all_relations)?;
        to_jsonl(&relations, out)?;
    }

    Ok(())
}

fn filter_target_relations(obj: &OsmObj) -> bool {
    filter_all_relations(obj)
        && obj.tags().get("boundary").map_or(false, |boundary| {
            TARGET_BOUNDARY_TYPES.contains(&boundary.as_str())
        })
}

fn filter_all_relations(obj: &OsmObj) -> bool {
    obj.is_relation()
        && obj.tags().contains_key("name")
        && obj.tags().get("admin_level").map_or(false, |admin_level| {
            matches!(admin_level.as_str(), "2" | "4" | "6" | "7" | "8")
        })
}

fn load_relations<F>(path: PathBuf, pred: F) -> Result<BTreeMap<OsmId, OsmObj>>
where
    F: FnMut(&OsmObj) -> bool,
{
    let f = std::fs::File::open(path)?;
    let mut pbf = OsmPbfReader::new(f);
    let relations = pbf.get_objs_and_deps(pred)?;
    Ok(relations)
}

fn to_stats(relations: &BTreeMap<OsmId, OsmObj>, mut out: impl io::Write) -> Result<()> {
    let mut boundary_types = HashMap::<&str, usize>::new();

    for boundary in relations
        .values()
        .filter(|obj| filter_all_relations(obj))
        .filter_map(|obj| obj.tags().get("boundary"))
    {
        *boundary_types.entry(boundary).or_default() += 1;
    }

    for (boundary_type, count) in boundary_types.iter().sorted_by(|a, b| Ord::cmp(&b.1, &a.1)) {
        writeln!(out, "{boundary_type} {count}")?;
    }

    Ok(())
}

fn to_jsonl(relations: &BTreeMap<OsmId, OsmObj>, out: impl io::Write) -> Result<()> {
    // Use a buffered writer to amortize flushes.
    let mut buffer = BufWriter::new(out);

    for relation in relations
        .values()
        .filter(|obj| filter_target_relations(obj))
    {
        let serialized = to_string(&relation)?;
        writeln!(buffer, "{serialized}")?;
    }

    Ok(())
}
