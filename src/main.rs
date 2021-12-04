use anyhow::{bail, Result};
use byteorder::{BigEndian, ByteOrder};
use clap::Parser;
use indicatif::ProgressBar;
use rayon::prelude::*;
use std::fs::{self, OpenOptions};
use std::io::{Cursor, Write};
use std::path::Path;

macro_rules! nbt_as {
    // I'm not sure if path is the right type here.
    // It works though!
    ($e:expr, $p:path) => {
        match $e {
            $p(val) => val,
            _ => panic!(concat!("Could not parse nbt value as ", stringify!($p))),
        }
    };
}

#[derive(Parser)]
#[clap(version = "1.0", author = "StackDoubleFlow <ojaslandge@gmail.com>")]
struct Opts {
    /// World region directory to read
    world_folder: String,
    /// Output file for sign data
    output_file: String,
}

fn main() -> Result<()> {
    let opts = Opts::parse();

    if !Path::new(&opts.world_folder).is_dir() {
        bail!("World input is not a directory!");
    }

    let bar = ProgressBar::new(fs::read_dir(&opts.world_folder)?.count() as u64);
    let output = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(opts.output_file)?;

    fs::read_dir(opts.world_folder)?.par_bridge().for_each_init(
        || (bar.clone(), output.try_clone().unwrap()),
        |(bar, output), entry| {
            bar.inc(1);
            let path = entry.unwrap().path();
            let data = fs::read(path).unwrap();
            for i in (0..4096).step_by(4) {
                let entry = BigEndian::read_u32(&data[i..]);
                let offset = (((entry >> 8) & 0xFFFFFF) * 4096) as usize;
                let size = (entry & 0xFF) * 4096;
                if offset == 0 && size == 0 {
                    continue;
                }
                let length = BigEndian::read_u32(&data[offset..]) as usize;
                let compression_type = data[offset + 4];
                let mut data = Cursor::new(&data[offset + 5..offset + 4 + length]);
                let nbt = match compression_type {
                    1 => nbt::Blob::from_gzip_reader(&mut data).unwrap(),
                    2 => nbt::Blob::from_zlib_reader(&mut data).unwrap(),
                    3 => nbt::Blob::from_reader(&mut data).unwrap(),
                    _ => panic!("Invalid compression type"),
                };

                let level = nbt_as!(&nbt["Level"], nbt::Value::Compound);
                let tile_entities = nbt_as!(&level["TileEntities"], nbt::Value::List);
                for val in tile_entities {
                    let te = nbt_as!(val, nbt::Value::Compound);
                    let id = nbt_as!(&te["id"], nbt::Value::String);
                    if id.contains("sign") {
                        let t1 = nbt_as!(&te["Text1"], nbt::Value::String);
                        let t2 = nbt_as!(&te["Text2"], nbt::Value::String);
                        let t3 = nbt_as!(&te["Text3"], nbt::Value::String);
                        let t4 = nbt_as!(&te["Text4"], nbt::Value::String);
                        let x = *nbt_as!(&te["x"], nbt::Value::Int);
                        let y = *nbt_as!(&te["y"], nbt::Value::Int);
                        let z = *nbt_as!(&te["z"], nbt::Value::Int);
                        writeln!(
                            output,
                            "({}, {}, {}): '{}','{}','{}','{}'",
                            x, y, z, t1, t2, t3, t4
                        )
                        .unwrap();
                    }
                }
            }
        },
    );
    bar.finish();

    Ok(())
}
