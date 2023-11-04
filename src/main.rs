use std::{
    collections::{BTreeMap, HashMap},
    io::{stdout, BufWriter, Write},
    path::PathBuf,
};

use anyhow::{Ok, Result};
use clap::{Parser, Subcommand};
use itertools::Itertools;
use osmpbfreader::{OsmId, OsmObj, OsmPbfReader};
use serde_json::to_string;

const TARGET_BOUNDARY_TYPES: &[&str] = &[
    "administrative",
    "state_border",
    "country_border",
    "state border",
];

#[derive(Parser)]
#[command(name = "osmpbf-filter")]
#[command(bin_name = "osmpbf-filter")]
struct Cli {
    #[arg(short, long)]
    in_file: PathBuf,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    Stats {},
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    eprintln!("Unpacking relations from {:?}", cli.in_file);
    let relations: BTreeMap<OsmId, OsmObj>;

    match &cli.command {
        Some(Commands::Stats {}) => {
            relations = load_relations(cli.in_file, filter_target_relations)?;
            eprintln!("Gathering some stats..");
            to_stats(&relations);
        }
        None => {
            relations = load_relations(cli.in_file, filter_all_relations)?;
            to_jsonl(&relations)?;
        }
    }

    Ok(())
}

fn filter_target_relations(obj: &OsmObj) -> bool {
    filter_all_relations(obj)
        && (obj.tags().get("boundary").map_or(false, |boundary| {
            TARGET_BOUNDARY_TYPES.contains(&boundary.as_str())
        }))
}

fn filter_all_relations(obj: &OsmObj) -> bool {
    obj.is_relation() && obj.tags().contains_key("name") && {
        if let Some(admin_level) = obj.tags().get("admin_level") {
            admin_level == "2"
                || admin_level == "4"
                || admin_level == "6"
                || admin_level == "7"
                || admin_level == "8"
        } else {
            false
        }
    }
}

fn load_relations<F>(path: PathBuf, pred: F) -> Result<BTreeMap<osmpbfreader::OsmId, OsmObj>>
where
    F: FnMut(&OsmObj) -> bool,
{
    let f = std::fs::File::open(path)?;
    let mut pbf = OsmPbfReader::new(f);
    let relations = pbf.get_objs_and_deps(pred)?;
    Ok(relations)
}

fn to_stats(relations: &BTreeMap<OsmId, OsmObj>) {
    let mut boundary_types = HashMap::new();

    for relation in relations.values().filter(|obj| filter_all_relations(obj)) {
        let boundary = relation.tags().get("boundary");
        if let Some(count) = boundary_types.get_mut(&boundary) {
            *count += 1;
        } else {
            boundary_types.insert(boundary, 1);
        }
    }

    for (boundary_type, count) in boundary_types.iter().sorted_by(|a, b| Ord::cmp(&b.1, &a.1)) {
        if let Some(b_type) = boundary_type {
            println!("{b_type} {count}");
        }
    }
}

fn to_jsonl(relations: &BTreeMap<OsmId, OsmObj>) -> Result<()> {
    let mut buffer = BufWriter::new(stdout());

    for relation in relations
        .values()
        .filter(|obj| filter_target_relations(obj))
    {
        let serialized = to_string(&relation)?;
        writeln!(buffer, "{serialized}")?;
    }

    buffer.flush()?;

    Ok(())
}