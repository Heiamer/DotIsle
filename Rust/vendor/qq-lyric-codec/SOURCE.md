# QRC block codec

From SuJiKiNen/LyricDecoder, commit 36fc1eaf31f8ef14a23de6f9f1427ce04178a900.
Source: https://github.com/SuJiKiNen/LyricDecoder/tree/36fc1eaf31f8ef14a23de6f9f1427ce04178a900/LyricDecoder/LyricDecoder/QQMusicDES

Their headers credit Brad Conte and wangqr. DotIsle changed two left-shift operands
in des.c to unsigned WORD to avoid undefined signed overflow under Zig's checks.
The repository is MIT licensed; its complete license accompanies these files.
DotIsle uses only its block codec. Rust handles bounded input and zlib decompression.
This is a nonstandard QQ lyric cipher, not a general cryptographic implementation.
