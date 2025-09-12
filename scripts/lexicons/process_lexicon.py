#!/usr/bin/env python3
import argparse

def merge_wordlists(input_files, output_file):
    words = set()

    # Read all input files
    for file in input_files:
        with open(file, "r", encoding="utf-8") as f:
            for line in f:
                word = line.strip()
                if word:
                    words.add(word)

    # Sort words alphabetically (optional, remove if you want original order)
    sorted_words = sorted(words)

    # Write to output file
    with open(output_file, "w", encoding="utf-8") as f:
        for word in sorted_words:
            f.write(word + "\n")

    print(f"✅ Merged {len(input_files)} files into '{output_file}' with {len(sorted_words)} unique words.")

def main():
    parser = argparse.ArgumentParser(
        description="Merge two or more .txt wordlists into one, removing duplicates."
    )
    parser.add_argument("inputs", nargs="+", help="Input .txt files")
    parser.add_argument("-o", "--output", required=True, help="Output .txt file")

    args = parser.parse_args()
    merge_wordlists(args.inputs, args.output)

if __name__ == "__main__":
    main()
