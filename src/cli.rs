use clap::Parser;

const MANUAL: &str = "
Example input:

  </home/alice/notes.txt
  >/home/bob/notes.txt
  <documents
  >../backup/docs

  This input would be interpreted as:

  1. Move `/home/alice/notes.txt` to `/home/bob/notes.txt`
  2. Move `documents` to `../backup/docs`

Input format:

  1. Each input line contains a single instruction.
  2. Instruction must start with either `<` or `>` character.
     `<` is followed by an input path.
     `>` is followed by an output path.
  3. Paths may be absolute or relative.
     Relative paths a resolved to the current working directory.
  4. Input path must be an existing file or directory.
     Output path may not exist.
  5. Existing output path must be of the same type as the input path.
     In other words, both paths must be either file or directory.
  6. Empty paths are not allowed.
  7. Breaking any of these rules will result in error.

Interpretation:

  1. After reading `>` instruction, an operation is performed between
     the last known input/output path.
  2. The default operation is to move the file or directory.
     If both paths are on the same device, this will result in rename.
     If both paths are on different devices, the item will be copied
     on the output device and then deleted from the input device.
  3. The copy operation can be enabled using the `-c, --copy` flag,
     Directories are copied recursively with their content.
  4. If the destination directory is non-empty, the source directory will
     be merged with it. This means that only the files that exists in both
     directories will be overwritten. This rule is applied recursively
     for subdirectories.
  5. Any non-existent directories in the output path are automatically created.
";

/// Move/copy files using instructions from standard input.
#[derive(Parser)]
#[clap(
    verbatim_doc_comment,
    after_help = "Use `--help` to get information about the input format.",
    after_long_help = MANUAL.trim(),
)]
pub struct Cli {
    /// Line delimiter is NUL, not newline.
    #[clap(short = '0', long)]
    pub null: bool,

    /// Copy files instead of moving them.
    #[clap(short, long)]
    pub copy: bool,

    /// Enable verbose output.
    #[clap(short, long)]
    pub verbose: bool,
}
