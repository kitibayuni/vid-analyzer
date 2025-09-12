import pandas as pd

# Load the TSV file
tsv_file = "worrywords-v1.txt"  # replace with your file path
df = pd.read_csv(tsv_file, sep="\t")

# Keep only the first 4 columns
df = df[['Term', 'Mean', 'OrdinalClass', 'MajorityLabel']]

# Save as CSV
csv_file = "worry_words.csv"  # desired output filename
df.to_csv(csv_file, index=False)

print(f"Converted TSV to CSV, keeping only first 4 columns: {csv_file}")
