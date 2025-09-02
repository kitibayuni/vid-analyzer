import pandas as pd
import matplotlib.pyplot as plt
import argparse

def main(csv_file):
    # Load the CSV file
    data = pd.read_csv(csv_file)

    # Display the first few rows of the DataFrame
    print(data.head())

    # Handle missing values: You can choose to drop them or fill them
    data = data.dropna()  # This will remove rows with any missing values
    # Alternatively, you could fill missing values with a specific value, e.g., 0
    # data = data.fillna(0)

    # Set the figure size
    plt.figure(figsize=(12, 6))

    # Create a scatter plot for each channel
    for c in range(1, len(data.columns)):  # Assuming the first column is 'time_sec'
        plt.scatter(data['time_sec'], data[f'chan{c}_pitch_hz'], label=f'Channel {c}', alpha=0.6, s=10)

    # Add titles and labels
    plt.title('Pitch Scatter Plot Over Time')
    plt.xlabel('Time (seconds)')
    plt.ylabel('Pitch (Hz)')
    plt.legend()
    plt.grid()

    # Save the plot as a PNG file
    plt.savefig('pitch_scatter_plot.png')  # Save the plot as a PNG file
    print("Plot saved as 'pitch_scatter_plot.png'")

if __name__ == "__main__":
    # Set up argument parsing
    parser = argparse.ArgumentParser(description='Analyze pitch data from a CSV file.')
    parser.add_argument('csv_file', type=str, help='Path to the input CSV file')

    # Parse the arguments
    args = parser.parse_args()

    # Call the main function with the provided CSV file
    main(args.csv_file)
