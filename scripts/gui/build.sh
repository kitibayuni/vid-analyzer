#!/usr/bin/env bash
set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
PROJECT_NAME="modular_video_player"
BUILD_TYPE="release"  # default build type
CLEAN_BUILD=false
RUN_TESTS=false
VERBOSE=false

# Parse command line arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        -d|--debug)
            BUILD_TYPE="debug"
            shift
            ;;
        -r|--release)
            BUILD_TYPE="release"
            shift
            ;;
        -p|--performance)
            BUILD_TYPE="performance"
            shift
            ;;
        -c|--clean)
            CLEAN_BUILD=true
            shift
            ;;
        -t|--test)
            RUN_TESTS=true
            shift
            ;;
        -v|--verbose)
            VERBOSE=true
            shift
            ;;
        -h|--help)
            echo "Usage: $0 [OPTIONS]"
            echo ""
            echo "Options:"
            echo "  -d, --debug       Build debug version"
            echo "  -r, --release     Build release version (default)"
            echo "  -p, --performance Build performance-optimized version"
            echo "  -c, --clean       Clean before building"
            echo "  -t, --test        Run tests after building"
            echo "  -v, --verbose     Verbose output"
            echo "  -h, --help        Show this help message"
            echo ""
            echo "Examples:"
            echo "  $0                # Build release version"
            echo "  $0 -d -c          # Clean and build debug version"
            echo "  $0 -p -t          # Build performance version and run tests"
            exit 0
            ;;
        *)
            echo -e "${RED}Unknown option: $1${NC}"
            exit 1
            ;;
    esac
done

# Print configuration
echo -e "${BLUE}Building $PROJECT_NAME${NC}"
echo -e "Build type: ${YELLOW}$BUILD_TYPE${NC}"
echo -e "Clean build: ${YELLOW}$CLEAN_BUILD${NC}"
echo ""

# Check if we're in the right directory
if [[ ! -f "Makefile" ]]; then
    echo -e "${RED}Error: Makefile not found. Please run this script from the project root.${NC}"
    exit 1
fi

# Check dependencies
echo -e "${BLUE}Checking dependencies...${NC}"
if make check-deps > /dev/null 2>&1; then
    echo -e "${GREEN}All dependencies found!${NC}"
else
    echo -e "${RED}Missing dependencies. Run:${NC}"
    echo -e "${YELLOW}  make check-deps${NC}"
    echo ""
    echo -e "${BLUE}To install dependencies:${NC}"
    if [[ -f /etc/debian_version ]]; then
        echo -e "${YELLOW}  make install-deps-ubuntu${NC}"
    elif [[ -f /etc/fedora-release ]] || [[ -f /etc/redhat-release ]]; then
        echo -e "${YELLOW}  make install-deps-fedora${NC}"
    elif [[ "$(uname)" == "Darwin" ]]; then
        echo -e "${YELLOW}  make install-deps-macos${NC}"
    fi
    exit 1
fi

# Clean if requested
if [[ "$CLEAN_BUILD" == true ]]; then
    echo -e "${BLUE}Cleaning previous build...${NC}"
    make clean
    echo -e "${GREEN}Clean complete!${NC}"
fi

# Build the project
echo -e "${BLUE}Building $PROJECT_NAME ($BUILD_TYPE)...${NC}"
start_time=$(date +%s)

if [[ "$VERBOSE" == true ]]; then
    make "$BUILD_TYPE"
else
    make "$BUILD_TYPE" > build.log 2>&1 || {
        echo -e "${RED}Build failed! Check build.log for details.${NC}"
        tail -20 build.log
        exit 1
    }
fi

end_time=$(date +%s)
build_time=$((end_time - start_time))

echo -e "${GREEN}Build complete! (${build_time}s)${NC}"

# Check if binary was created
if [[ -f "$PROJECT_NAME" ]]; then
    echo -e "${GREEN}Binary created: $PROJECT_NAME${NC}"
    ls -lh "$PROJECT_NAME"
else
    echo -e "${RED}Error: Binary not found after build${NC}"
    exit 1
fi

# Run tests if requested
if [[ "$RUN_TESTS" == true ]]; then
    if [[ -d "tests" ]]; then
        echo -e "${BLUE}Running tests...${NC}"
        # Add test running logic here when tests are implemented
        echo -e "${YELLOW}Test framework not yet implemented${NC}"
    else
        echo -e "${YELLOW}No tests directory found${NC}"
    fi
fi

# Show build information
echo ""
echo -e "${BLUE}Build Information:${NC}"
echo -e "Binary: ${GREEN}$PROJECT_NAME${NC}"
echo -e "Build type: ${YELLOW}$BUILD_TYPE${NC}"
echo -e "Build time: ${YELLOW}${build_time}s${NC}"

if [[ -f "$PROJECT_NAME" ]]; then
    size=$(ls -lh "$PROJECT_NAME" | awk '{print $5}')
    echo -e "Binary size: ${YELLOW}$size${NC}"
fi

echo ""
echo -e "${GREEN}Ready to run: ./$PROJECT_NAME${NC}"

# Optional: Show recent changes in git (if in git repo)
if [[ -d ".git" ]]; then
    echo ""
    echo -e "${BLUE}Recent changes:${NC}"
    git log --oneline -5 2>/dev/null || echo "Git information not available"
fi