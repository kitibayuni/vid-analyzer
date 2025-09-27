#include <iostream>
#include <fstream>
#include <sstream>
#include <vector>
#include <string>
#include <iomanip>

// Phrase structure
struct Phrase {
    double start;
    double end;
    std::string text;
};

int main(int argc, char* argv[]) {
    if (argc < 2) {
        std::cerr << "Usage: " << argv[0] << " input_words.csv [output_phrases.csv] [gap_seconds]" << std::endl;
        return 1;
    }

    std::string input_file = argv[1];
    std::string output_file = (argc >= 3) ? argv[2] : input_file.substr(0, input_file.find_last_of('.')) + "_phrases.csv";
    double gap_threshold = (argc >= 4) ? std::stod(argv[3]) : 0.5; // default 0.5s

    // Read word-level CSV
    std::ifstream fin(input_file);
    if (!fin.is_open()) {
        std::cerr << "Failed to open input file: " << input_file << std::endl;
        return 1;
    }

    std::vector<std::pair<double, std::string>> words;
    std::string line;
    std::getline(fin, line); // skip header

    while (std::getline(fin, line)) {
        std::stringstream ss(line);
        std::string time_str, word;
        if (!std::getline(ss, time_str, ',')) continue;
        if (!std::getline(ss, word, ',')) continue;

        try {
            double t = std::stod(time_str);
            if (!word.empty()) {
                words.emplace_back(t, word);
            }
        } catch (...) {
            continue; // skip invalid lines
        }
    }
    fin.close();

    if (words.empty()) {
        std::cerr << "No words found in input CSV." << std::endl;
        return 1;
    }

    // Group into phrases
    std::vector<Phrase> phrases;
    Phrase current;
    current.start = words[0].first;
    current.end = words[0].first;
    current.text = words[0].second;

    for (size_t i = 1; i < words.size(); ++i) {
        double time_gap = words[i].first - current.end;
        bool ends_sentence = !words[i-1].second.empty() &&
                             (words[i-1].second.back() == '.' || words[i-1].second.back() == '!' || words[i-1].second.back() == '?');

        if (time_gap > gap_threshold || ends_sentence) {
            // Save current phrase
            phrases.push_back(current);
            current.start = words[i].first;
            current.text.clear();
        }

        if (!current.text.empty()) current.text += " ";
        current.text += words[i].second;
        current.end = words[i].first;
    }
    // Save last phrase
    phrases.push_back(current);

    // Write phrase-level CSV
    std::ofstream fout(output_file);
    if (!fout.is_open()) {
        std::cerr << "Failed to open output file: " << output_file << std::endl;
        return 1;
    }

    fout << "start_sec,end_sec,phrase\n";
    fout << std::fixed << std::setprecision(4);

    for (const auto& p : phrases) {
        fout << p.start << "," << p.end << "," << p.text << "\n";
    }

    std::cout << "Phrase-level CSV written to " << output_file << " (" << phrases.size() << " phrases)" << std::endl;
    return 0;
}
