#include <opencv2/opencv.hpp>
#include <opencv2/highgui.hpp>
#include <iostream>
#include <string>
#include <thread>
#include <chrono>
#include <atomic>
#include <mutex>
#include <sstream>
#include <iomanip>

#ifdef _WIN32
#include <windows.h>
#include <commdlg.h>
#elif defined(__linux__)
#include <cstdlib>
#include <fstream>
#elif defined(__APPLE__)
#include <cstdlib>
#endif

class VideoPlayer {
private:
    cv::VideoCapture cap;
    cv::Mat currentFrame;      // Only modified by worker thread
    cv::Mat displayFrame;      // Only modified by main thread
    std::string windowName;
    
    // Video properties
    int totalFrames;
    int currentFrameIndex;
    double fps;
    double totalDuration;
    
    // Thread control
    std::atomic<bool> isPlaying;
    std::atomic<bool> shouldStop;
    std::atomic<bool> videoLoaded;
    std::atomic<bool> seekRequested;
    std::atomic<int> seekToFrame;
    
    std::mutex captureMutex;    // Protects VideoCapture access
    std::mutex frameMutex;      // Protects currentFrame
    std::thread workerThread;
    
    // Display properties
    int videoWidth, videoHeight;
    int timelineHeight;
    int buttonHeight;
    int totalDisplayHeight;
    
    // UI colors
    cv::Scalar bgColor;
    cv::Scalar timelineColor;
    cv::Scalar progressColor;
    cv::Scalar buttonColor;
    cv::Scalar textColor;
    
    std::string openFileDialog() {
        std::string filename = "";
        
#ifdef _WIN32
        // Windows file dialog
        OPENFILENAME ofn;
        char szFile[260] = {0};
        
        ZeroMemory(&ofn, sizeof(ofn));
        ofn.lStructSize = sizeof(ofn);
        ofn.lpstrFile = szFile;
        ofn.nMaxFile = sizeof(szFile);
        ofn.lpstrFilter = "Video Files\0*.mp4;*.avi;*.mov;*.mkv;*.wmv;*.flv;*.webm\0All Files\0*.*\0";
        ofn.nFilterIndex = 1;
        ofn.lpstrFileTitle = NULL;
        ofn.nMaxFileTitle = 0;
        ofn.lpstrInitialDir = NULL;
        ofn.Flags = OFN_PATHMUSTEXIST | OFN_FILEMUSTEXIST;
        
        if (GetOpenFileName(&ofn)) {
            filename = std::string(szFile);
        }
        
#elif defined(__linux__)
        // Linux file dialog using zenity
        std::string command = "zenity --file-selection --title=\"Select Video File\" --file-filter=\"Video files | *.mp4 *.avi *.mov *.mkv *.wmv *.flv *.webm\" 2>/dev/null";
        
        FILE* pipe = popen(command.c_str(), "r");
        if (pipe) {
            char buffer[1024];
            if (fgets(buffer, sizeof(buffer), pipe) != nullptr) {
                filename = std::string(buffer);
                // Remove newline character
                if (!filename.empty() && filename.back() == '\n') {
                    filename.pop_back();
                }
            }
            pclose(pipe);
        } else {
            // Fallback: simple console input
            std::cout << "Zenity not available. Please enter video file path: " << std::flush;
            std::getline(std::cin, filename);
        }
        
#elif defined(__APPLE__)
        // macOS file dialog using osascript
        std::string command = "osascript -e 'tell application \"System Events\" to return (choose file with prompt \"Select Video File\" of type {\"mp4\", \"avi\", \"mov\", \"mkv\", \"wmv\", \"flv\", \"webm\"} as string)' 2>/dev/null";
        
        FILE* pipe = popen(command.c_str(), "r");
        if (pipe) {
            char buffer[1024];
            if (fgets(buffer, sizeof(buffer), pipe) != nullptr) {
                filename = std::string(buffer);
                // Remove newline character
                if (!filename.empty() && filename.back() == '\n') {
                    filename.pop_back();
                }
                // Convert macOS path format
                if (filename.find("Macintosh HD:") == 0) {
                    std::replace(filename.begin(), filename.end(), ':', '/');
                    filename = "/" + filename.substr(13); // Remove "Macintosh HD"
                }
            }
            pclose(pipe);
        } else {
            // Fallback: simple console input
            std::cout << "AppleScript not available. Please enter video file path: " << std::flush;
            std::getline(std::cin, filename);
        }
        
#else
        // Generic fallback
        std::cout << "File dialog not supported on this platform. Please enter video file path: " << std::flush;
        std::getline(std::cin, filename);
#endif
        
        return filename;
    }
    
    std::string formatTime(double seconds) {
        if (seconds < 0) seconds = 0;
        int mins = static_cast<int>(seconds) / 60;
        int secs = static_cast<int>(seconds) % 60;
        
        std::stringstream ss;
        ss << mins << ":" << std::setfill('0') << std::setw(2) << secs;
        return ss.str();
    }
    
    // Worker thread function - handles video decoding only
    void workerThreadFunction() {
        auto frameInterval = std::chrono::duration<double>(1.0 / fps);
        auto lastFrameTime = std::chrono::steady_clock::now();
        
        while (!shouldStop) {
            if (isPlaying && videoLoaded) {
                auto currentTime = std::chrono::steady_clock::now();
                
                if (currentTime - lastFrameTime >= frameInterval) {
                    cv::Mat newFrame;
                    
                    {
                        std::lock_guard<std::mutex> lock(captureMutex);
                        
                        // Handle seek requests
                        if (seekRequested) {
                            cap.set(cv::CAP_PROP_POS_FRAMES, seekToFrame);
                            seekRequested = false;
                        }
                        
                        cap >> newFrame;
                        currentFrameIndex = static_cast<int>(cap.get(cv::CAP_PROP_POS_FRAMES)) - 1;
                    }
                    
                    if (newFrame.empty()) {
                        // End of video - stop playback
                        isPlaying = false;
                        {
                            std::lock_guard<std::mutex> lock(captureMutex);
                            cap.set(cv::CAP_PROP_POS_FRAMES, 0);
                            cap >> newFrame;
                            currentFrameIndex = 0;
                        }
                    }
                    
                    if (!newFrame.empty()) {
                        // Resize if needed and update currentFrame
                        cv::Mat resizedFrame;
                        if (newFrame.cols != videoWidth || newFrame.rows != videoHeight) {
                            cv::resize(newFrame, resizedFrame, cv::Size(videoWidth, videoHeight));
                        } else {
                            resizedFrame = newFrame;
                        }
                        
                        {
                            std::lock_guard<std::mutex> lock(frameMutex);
                            currentFrame = resizedFrame.clone();
                        }
                    }
                    
                    lastFrameTime = currentTime;
                }
            }
            
            // Small sleep to prevent excessive CPU usage
            std::this_thread::sleep_for(std::chrono::milliseconds(10));
        }
    }
    
    void createInitialDisplay() {
        displayFrame = cv::Mat::zeros(totalDisplayHeight, videoWidth, CV_8UC3);
        displayFrame.setTo(bgColor);
        
        // Draw placeholder text
        std::string text = "Click 'Load Video' to start";
        int baseLine;
        cv::Size textSize = cv::getTextSize(text, cv::FONT_HERSHEY_SIMPLEX, 1.0, 2, &baseLine);
        cv::Point textOrg((videoWidth - textSize.width) / 2, (videoHeight + textSize.height) / 2);
        cv::putText(displayFrame, text, textOrg, cv::FONT_HERSHEY_SIMPLEX, 1.0, textColor, 2);
        
        drawControls();
    }
    
    void updateDisplay() {
        // Create display frame
        displayFrame = cv::Mat::zeros(totalDisplayHeight, videoWidth, CV_8UC3);
        
        if (videoLoaded) {
            // Copy current frame (thread-safe)
            cv::Mat frameCopy;
            {
                std::lock_guard<std::mutex> lock(frameMutex);
                if (!currentFrame.empty()) {
                    frameCopy = currentFrame.clone();
                }
            }
            
            if (!frameCopy.empty()) {
                cv::Rect videoRect(0, 0, videoWidth, videoHeight);
                frameCopy.copyTo(displayFrame(videoRect));
            }
        } else {
            // Show placeholder
            std::string text = "Click 'Load Video' to start";
            int baseLine;
            cv::Size textSize = cv::getTextSize(text, cv::FONT_HERSHEY_SIMPLEX, 1.0, 2, &baseLine);
            cv::Point textOrg((videoWidth - textSize.width) / 2, (videoHeight + textSize.height) / 2);
            cv::putText(displayFrame, text, textOrg, cv::FONT_HERSHEY_SIMPLEX, 1.0, textColor, 2);
        }
        
        drawControls();
    }
    
    void drawControls() {
        int controlsY = videoHeight;
        
        // Draw timeline background
        cv::Rect timelineRect(10, controlsY + 10, videoWidth - 20, 20);
        cv::rectangle(displayFrame, timelineRect, timelineColor, -1);
        cv::rectangle(displayFrame, timelineRect, textColor, 1);
        
        if (videoLoaded && totalFrames > 0) {
            // Draw progress
            double progress = static_cast<double>(currentFrameIndex) / totalFrames;
            int progressWidth = static_cast<int>((videoWidth - 20) * progress);
            cv::Rect progressRect(10, controlsY + 10, progressWidth, 20);
            cv::rectangle(displayFrame, progressRect, progressColor, -1);
            
            // Draw time text
            std::string timeText = formatTime(currentFrameIndex / fps) + " / " + formatTime(totalDuration);
            cv::putText(displayFrame, timeText, cv::Point(15, controlsY + 25), 
                       cv::FONT_HERSHEY_SIMPLEX, 0.4, textColor, 1);
        }
        
        // Draw buttons
        int buttonsY = controlsY + timelineHeight;
        
        // Load button
        cv::Rect loadButton(10, buttonsY + 5, 80, 30);
        cv::rectangle(displayFrame, loadButton, buttonColor, -1);
        cv::rectangle(displayFrame, loadButton, textColor, 1);
        cv::putText(displayFrame, "Load Video", cv::Point(15, buttonsY + 23), 
                   cv::FONT_HERSHEY_SIMPLEX, 0.4, textColor, 1);
        
        // Play/Pause button
        cv::Rect playButton(100, buttonsY + 5, 80, 30);
        cv::rectangle(displayFrame, playButton, buttonColor, -1);
        cv::rectangle(displayFrame, playButton, textColor, 1);
        std::string playText = isPlaying ? "Pause" : "Play";
        cv::putText(displayFrame, playText, cv::Point(120, buttonsY + 23), 
                   cv::FONT_HERSHEY_SIMPLEX, 0.4, textColor, 1);
        
        // Instructions
        if (videoLoaded) {
            cv::putText(displayFrame, "Click timeline to seek", cv::Point(200, buttonsY + 23), 
                       cv::FONT_HERSHEY_SIMPLEX, 0.4, cv::Scalar(180, 180, 180), 1);
        }
    }
    
    static void onMouseClick(int event, int x, int y, int flags, void* userdata) {
        if (event != cv::EVENT_LBUTTONDOWN) return;
        
        VideoPlayer* player = static_cast<VideoPlayer*>(userdata);
        int controlsY = player->videoHeight;
        int buttonsY = controlsY + player->timelineHeight;
        
        // Check timeline click
        if (y >= controlsY + 10 && y <= controlsY + 30 && x >= 10 && x <= player->videoWidth - 10) {
            if (player->videoLoaded && player->totalFrames > 0) {
                double clickRatio = static_cast<double>(x - 10) / (player->videoWidth - 20);
                int targetFrame = static_cast<int>(clickRatio * player->totalFrames);
                player->seekToFrameRequest(targetFrame);
            }
            return;
        }
        
        // Check button clicks
        if (y >= buttonsY + 5 && y <= buttonsY + 35) {
            // Load button
            if (x >= 10 && x <= 90) {
                std::cout << "Opening file dialog..." << std::endl;
                
                std::thread([player]() {
                    std::string filename = player->openFileDialog();
                    
                    if (!filename.empty()) {
                        std::cout << "Selected file: " << filename << std::endl;
                        player->loadVideo(filename);
                    } else {
                        std::cout << "No file selected." << std::endl;
                    }
                }).detach();
            }
            // Play/Pause button
            else if (x >= 100 && x <= 180) {
                std::cout << "Play/Pause button clicked!" << std::endl;
                player->togglePlayPause();
            }
        }
    }
    
public:
    VideoPlayer() : windowName("Video Player"), 
                   totalFrames(0), currentFrameIndex(0), fps(30.0), totalDuration(0.0),
                   isPlaying(false), shouldStop(false), videoLoaded(false),
                   seekRequested(false), seekToFrame(0),
                   videoWidth(800), videoHeight(600), timelineHeight(40), buttonHeight(40),
                   bgColor(50, 50, 50), timelineColor(100, 100, 100), 
                   progressColor(0, 255, 0), buttonColor(70, 70, 70), textColor(255, 255, 255) {
        
        totalDisplayHeight = videoHeight + timelineHeight + buttonHeight;
        
        cv::namedWindow(windowName, cv::WINDOW_AUTOSIZE);
        cv::setMouseCallback(windowName, onMouseClick, this);
        
        createInitialDisplay();
        
        // Start worker thread
        workerThread = std::thread(&VideoPlayer::workerThreadFunction, this);
    }
    
    ~VideoPlayer() {
        shouldStop = true;
        if (workerThread.joinable()) {
            workerThread.join();
        }
        cv::destroyAllWindows();
    }
    
    bool loadVideo(const std::string& filename) {
        std::cout << "Attempting to load video: " << filename << std::endl;
        
        // Stop playback
        isPlaying = false;
        
        {
            std::lock_guard<std::mutex> lock(captureMutex);
            cap.release();
            cap.open(filename);
            
            if (!cap.isOpened()) {
                std::cerr << "Error: Could not open video file: " << filename << std::endl;
                videoLoaded = false;
                return false;
            }
            
            // Get video properties
            totalFrames = static_cast<int>(cap.get(cv::CAP_PROP_FRAME_COUNT));
            fps = cap.get(cv::CAP_PROP_FPS);
            
            if (fps <= 0) {
                fps = 30.0; // Default fallback
            }
            
            totalDuration = totalFrames / fps;
            currentFrameIndex = 0;
            
            // Get video dimensions
            int origWidth = static_cast<int>(cap.get(cv::CAP_PROP_FRAME_WIDTH));
            int origHeight = static_cast<int>(cap.get(cv::CAP_PROP_FRAME_HEIGHT));
            
            if (origWidth <= 0 || origHeight <= 0) {
                std::cerr << "Error: Invalid video dimensions." << std::endl;
                videoLoaded = false;
                return false;
            }
            
            // Scale video to fit if too large
            if (origWidth > 1200 || origHeight > 800) {
                double scale = std::min(1200.0 / origWidth, 800.0 / origHeight);
                videoWidth = static_cast<int>(origWidth * scale);
                videoHeight = static_cast<int>(origHeight * scale);
            } else {
                videoWidth = origWidth;
                videoHeight = origHeight;
            }
            
            totalDisplayHeight = videoHeight + timelineHeight + buttonHeight;
            
            std::cout << "Video loaded successfully!" << std::endl;
            std::cout << "Original resolution: " << origWidth << "x" << origHeight << std::endl;
            std::cout << "Display resolution: " << videoWidth << "x" << videoHeight << std::endl;
            std::cout << "Total frames: " << totalFrames << std::endl;
            std::cout << "FPS: " << fps << std::endl;
            std::cout << "Duration: " << formatTime(totalDuration) << std::endl;
            
            // Read first frame
            cv::Mat tempFrame;
            cap >> tempFrame;
            
            if (tempFrame.empty()) {
                std::cerr << "Error: Could not read first frame." << std::endl;
                videoLoaded = false;
                return false;
            }
            
            // Resize frame if necessary
            cv::Mat resizedFrame;
            if (tempFrame.cols != videoWidth || tempFrame.rows != videoHeight) {
                cv::resize(tempFrame, resizedFrame, cv::Size(videoWidth, videoHeight));
            } else {
                resizedFrame = tempFrame;
            }
            
            {
                std::lock_guard<std::mutex> lock(frameMutex);
                currentFrame = resizedFrame.clone();
            }
        }
        
        videoLoaded = true;
        
        // Resize window for new video
        cv::resizeWindow(windowName, videoWidth, totalDisplayHeight);
        
        std::cout << "Video ready for playback." << std::endl;
        return true;
    }
    
    void togglePlayPause() {
        if (!videoLoaded) {
            std::cout << "No video loaded!" << std::endl;
            return;
        }
        
        isPlaying = !isPlaying;
        
        if (isPlaying) {
            std::cout << "Playing..." << std::endl;
        } else {
            std::cout << "Paused." << std::endl;
        }
    }
    
    void seekToFrameRequest(int frameIndex) {
        if (!videoLoaded || frameIndex < 0 || frameIndex >= totalFrames) return;
        
        std::cout << "Seeking to frame: " << frameIndex << std::endl;
        
        // Pause playback during seek
        bool wasPlaying = isPlaying;
        isPlaying = false;
        
        {
            std::lock_guard<std::mutex> lock(captureMutex);
            cap.set(cv::CAP_PROP_POS_FRAMES, frameIndex);
            
            cv::Mat tempFrame;
            cap >> tempFrame;
            
            if (!tempFrame.empty()) {
                cv::Mat resizedFrame;
                if (tempFrame.cols != videoWidth || tempFrame.rows != videoHeight) {
                    cv::resize(tempFrame, resizedFrame, cv::Size(videoWidth, videoHeight));
                } else {
                    resizedFrame = tempFrame;
                }
                
                {
                    std::lock_guard<std::mutex> lock2(frameMutex);
                    currentFrame = resizedFrame.clone();
                }
                
                currentFrameIndex = frameIndex;
            }
        }
        
        // Resume playback if it was playing before
        isPlaying = wasPlaying;
        
        std::cout << "Seeked to frame: " << frameIndex << " (" << formatTime(frameIndex / fps) << ")" << std::endl;
    }
    
    void run() {
        std::cout << "\n=== Thread-Safe Video Player ===" << std::endl;
        std::cout << "Controls:" << std::endl;
        std::cout << "- Click 'Load Video' button to open file dialog" << std::endl;
        std::cout << "- Click 'Play/Pause' button to control playback" << std::endl;
        std::cout << "- Click anywhere on the timeline to seek" << std::endl;
        std::cout << "- Press ESC to quit, or close the window" << std::endl;
        std::cout << "================================\n" << std::endl;
        
        // Main thread loop - only handles GUI
        while (!shouldStop) {
            // Update display (main thread only)
            updateDisplay();
            
            // Process GUI events (main thread only)
            int key = cv::waitKey(30) & 0xFF;
            if (key == 27) { // ESC key
                std::cout << "ESC pressed - exiting..." << std::endl;
                break;
            }
            
            // Check if window was closed
            try {
                if (cv::getWindowProperty(windowName, cv::WND_PROP_VISIBLE) < 1) {
                    std::cout << "Window closed - exiting..." << std::endl;
                    break;
                }
            } catch (...) {
                // Window may have been destroyed
                break;
            }
            
            // Show the frame (main thread only)
            cv::imshow(windowName, displayFrame);
        }
        
        shouldStop = true;
        std::cout << "Goodbye!" << std::endl;
    }
};

int main() {
    try {
        VideoPlayer player;
        player.run();
    } catch (const std::exception& e) {
        std::cerr << "Error: " << e.what() << std::endl;
        return -1;
    }
    
    return 0;
}

// Compilation instructions:
// g++ -std=c++11 video_player.cpp -o video_player `pkg-config --cflags --libs opencv4`