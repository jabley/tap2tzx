use anyhow::anyhow;
use std::{
    env,
    fmt::Debug,
    fs::File,
    io::{self, BufWriter, Read, Write},
    path::{Path, PathBuf},
};

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = env::args().collect();

    match args.len() {
        2 => tap_to_tzx(&args[1], target(&args[1])),
        3 => tap_to_tzx(&args[1], &args[2]),
        _ => {
            println!("\nUsage: tap2tzx INPUT.TAP [OUTPUT.TZX]");
            std::process::exit(0);
        }
    }
}

/// Takes a path to a .tap file and returns the equivalent .tzx path
fn target<P>(tap_name: P) -> PathBuf
where
    P: AsRef<Path>,
{
    tap_name.as_ref().with_extension("tzx")
}

/// Takes a TAP path for an existing file and converts it to TZX format, writing the output to the TZX path
///
/// This will read the entire TAP file into memory, which should not be a problem since Spectrum files were so small
/// and it is expected that this conversion programme will be used to create things for an emulator, and have much
/// more resources available thatn a real Spectrum.
///
/// # Errors
///
/// Can error if the TAP and TZX path are the same, or there are problems reading or writing the output.
fn tap_to_tzx<I, O>(tap_path: I, tzx_path: O) -> anyhow::Result<()>
where
    I: AsRef<Path> + Debug,
    O: AsRef<Path> + Debug,
{
    if tzx_path.as_ref().exists()
        && tap_path.as_ref().canonicalize()? == tzx_path.as_ref().canonicalize()?
    {
        println!("Not overwriting input file {:?}", tap_path);
        return Ok(());
    }

    println!("Converting TAP {:?} to TZX at {:?}", tap_path, tzx_path);

    // Open the tap as read-only
    let mut fin = File::open(tap_path)?;

    // Open the tzx file as write, create, truncate with inherited r+w user+group
    let mut fout = BufWriter::new(File::create(tzx_path)?);

    // The file will be small (Spectrum 128k, anyone?) so read it all into memory for easier manipulation
    let mut tap: Vec<u8> = Vec::with_capacity(fin.metadata()?.len() as usize);
    fin.read_to_end(&mut tap)?;

    let block_count = tap2tzx(&tap, &mut fout)?;

    println!("\nSuccesfully converted {} blocks!", block_count);

    Ok(())
}

/// Converts the provided TAP bytes by writing to TZX format in the provided out Write.
///
/// Returns the number of non-empty TZX blocks written to the output.
///
/// Callers should typically provide a BufWriter for more efficient syscall usage.
fn tap2tzx<W>(tap: &[u8], tzx: &mut W) -> anyhow::Result<i32>
where
    W: Write,
{
    let size = tap.len() as usize;
    write_tzx_header(tzx)?;

    // loop through the tap file, reading each TAP block and writing TZX standard speed blocks to the output
    let mut pos: usize = 0;
    let mut block_count = 0;

    while pos < size {
        let block_len = read_le_u16(&mut &tap[pos as usize..], pos)?;

        pos += 2;

        if block_len != 0 {
            write_tzx_block(tap, pos, block_len, tzx)?;
        }

        pos += block_len as usize;
        block_count += 1;
    }

    tzx.flush()?;

    Ok(block_count)
}

/// Write the tzx file header magic bytes
fn write_tzx_header<W>(out: &mut W) -> io::Result<()>
where
    W: Write,
{
    // Magic start bytes plus version
    out.write_all(&[b'Z', b'X', b'T', b'a', b'p', b'e', b'!', 0x1A, 1, 20])
}

/// Writes a full TZX block to the output
fn write_tzx_block<W>(mem: &[u8], pos: usize, block_len: u16, out: &mut W) -> io::Result<()>
where
    W: Write,
{
    // Write the TZX block header

    //  0
    //  0 1 2 3 4
    // +-+-+-+-+-+
    // |I| P.| L.|
    // +-+-+-+-+-+
    //
    // I - Block ID. 10u8 for Standard speed data block
    // P - Pause after this block (ms.) {1000} (little endian}
    // L - Length of data that follow (little endian)
    //
    out.write_all(&[0x10, 0xE8, 0x03])?; // I and P
    out.write_all(&mem[pos - 2..pos])?; // length of data

    // Write the TZX block data
    out.write_all(&mem[pos..pos + block_len as usize])
}

/// Attempts to read a u16 from the provided slice.
///
/// Returns the u16 that was read if successful.
fn read_le_u16(input: &mut &[u8], pos: usize) -> anyhow::Result<u16> {
    // straight out of the language docs for how to read a u16 from a slice. See https://doc.rust-lang.org/std/primitive.u16.html#method.from_le_bytes
    let mid = std::mem::size_of::<u16>();
    if mid > input.len() {
        return Err(anyhow!(
            "Expected u16 but found u8 - malformed input at {}?",
            pos
        ));
    }
    let (int_bytes, rest) = input.split_at(mid);
    *input = rest;
    Ok(u16::from_le_bytes(int_bytes.try_into()?))
}

#[cfg(test)]
mod test {

    use crate::*;

    #[test]
    fn single_block() {
        let tap = [
            0x13, 0x00, 0x00, 0x00, 0x4D, 0x61, 0x6E, 0x69, 0x63, 0x4D, 0x69, 0x6E, 0x65, 0x72,
            0x45, 0x00, 0x0A, 0x00, 0x45, 0x00, 0x1F,
        ];
        let mut out = Vec::with_capacity(10 + 3 + tap.len()); // file header + block header + data
        let block_count = tap2tzx(&tap, &mut out).unwrap();

        assert_eq!(1, block_count, "We expect to have created a single block");

        // We expect to have the TZX file header, plus a single TZX block with header and data
        let file_header = [b'Z', b'X', b'T', b'a', b'p', b'e', b'!', 0x1A, 1, 20];
        let block_header = [0x10, 0xE8, 0x03]; // Standard speed data block with a pause of 1000ms
        let expected: Vec<u8> = file_header
            .iter()
            .chain(block_header.iter())
            .chain(tap.iter())
            .map(|v| *v)
            .collect();

        assert_eq!(expected, out, "unexpected tzx byte stream");
    }
}
