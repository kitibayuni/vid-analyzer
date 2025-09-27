# Makefile for group_phrases C++ program

# Compiler
CXX := g++
CXXFLAGS := -std=c++17 -O2 -Wall -Wextra

# Source and target
SRC := phrase_level.cpp
TARGET := phrase_level

# Default target
all: $(TARGET)

# Build target
$(TARGET): $(SRC)
	$(CXX) $(CXXFLAGS) -o $@ $^

# Clean build
clean:
	rm -f $(TARGET)

# Run example
run: $(TARGET)
	./$(TARGET) input_words.csv
