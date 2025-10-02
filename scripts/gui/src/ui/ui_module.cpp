#include "ui_module.h"
#include "media_controller.h"
#include <fstream>
#include <iomanip>
#include <iostream>
#include <opencv2/highgui.hpp>
#include <sstream>

#ifdef _WIN32
#include <commdlg.h>
#include <windows.h>
#elif defined(__linux__)
#include <cstdlib>
#elif defined(__APPLE__)
#include <cstdlib>
#endif

UIModule::UIModule()
    : windowName("High-Performance Video Player"),
      videoDisplayWidth(800), videoDisplayHeight(600),
      windowCreated(false), shouldExit(false), currentRowIndex(-1) {
}

UIModule::~UIModule() { shutdown(); }

bool UIModule::initialize(const UIConfig &cfg) {
  std::cout << "Initializing UIModule..." << std::endl;

  config = cfg;
  // Video area takes up windowWidth minus the right panel
  videoDisplayWidth = config.windowWidth - config.rightPanelWidth;
  videoDisplayHeight = config.windowHeight;
  totalDisplayHeight =
      videoDisplayHeight + config.timelineHeight + config.buttonHeight;

  // Create OpenCV window with full width (video + panel)
  cv::namedWindow(windowName, cv::WINDOW_AUTOSIZE);
  cv::setMouseCallback(windowName, onMouseClick, this);
  windowCreated = true;

  createInitialDisplay();

  std::cout << "UIModule initialized successfully" << std::endl;
  std::cout << "Window size: " << config.windowWidth << "x" << totalDisplayHeight
            << std::endl;

  return true;
}

void UIModule::setCallbacks(const UICallbacks &cb) { callbacks = cb; }

void UIModule::createInitialDisplay() {
  // Create display frame with full width (video + right panel)
  int totalWindowWidth = videoDisplayWidth + config.rightPanelWidth;
  displayFrame = cv::Mat::zeros(totalDisplayHeight, totalWindowWidth, CV_8UC3);
  displayFrame.setTo(config.bgColor);

  std::string text = "Click 'Load Video' to start";
  int baseLine;
  cv::Size textSize =
      cv::getTextSize(text, cv::FONT_HERSHEY_SIMPLEX, 1.0, 2, &baseLine);
  cv::Point textOrg((videoDisplayWidth - textSize.width) / 2,
                    (videoDisplayHeight + textSize.height) / 2);
  cv::putText(displayFrame, text, textOrg, cv::FONT_HERSHEY_SIMPLEX, 1.0,
              config.textColor, 2);

  // Draw initial controls
  MediaStats emptyStats;
  drawControls(emptyStats, 0.0);

  cv::imshow(windowName, displayFrame);
}

bool UIModule::update(const cv::Mat &videoFrame, const MediaStats &stats,
                      double duration) {
  if (!windowCreated)
    return false;

  // Create new display frame with full width (video + right panel)
  int totalWindowWidth = videoDisplayWidth + config.rightPanelWidth;
  displayFrame = cv::Mat::zeros(totalDisplayHeight, totalWindowWidth, CV_8UC3);

  // Draw video frame or placeholder
  if (!videoFrame.empty()) {
    drawVideoFrame(videoFrame);
  } else {
    // Show placeholder
    displayFrame.setTo(config.bgColor);

    std::string message;
    switch (stats.state) {
    case MediaState::LOADING:
      message = "Loading...";
      break;
    case MediaState::READY:
      message = "Ready - Click Play";
      break;
    case MediaState::ERROR:
      message = "Error occurred";
      break;
    default:
      message = "Click 'Load Video' to start";
      break;
    }

    int baseLine;
    cv::Size textSize =
        cv::getTextSize(message, cv::FONT_HERSHEY_SIMPLEX, 1.0, 2, &baseLine);
    cv::Point textOrg((videoDisplayWidth - textSize.width) / 2,
                      (videoDisplayHeight + textSize.height) / 2);
    cv::putText(displayFrame, message, textOrg, cv::FONT_HERSHEY_SIMPLEX, 1.0,
                config.textColor, 2);
  }

  // Draw controls
  drawControls(stats, duration);

  // Update display
  cv::imshow(windowName, displayFrame);

  // Check if window is still open
  try {
    if (cv::getWindowProperty(windowName, cv::WND_PROP_VISIBLE) < 1) {
      shouldExit = true;
      return false;
    }
  } catch (...) {
    shouldExit = true;
    return false;
  }

  return true;
}

void UIModule::drawVideoFrame(const cv::Mat &frame) {
  if (frame.empty())
    return;

  cv::Rect videoRect(0, 0, videoDisplayWidth, videoDisplayHeight);

  if (frame.cols == videoDisplayWidth && frame.rows == videoDisplayHeight) {
    // Frame matches display size exactly
    frame.copyTo(displayFrame(videoRect));
  } else {
    // Scale frame to fit display area
    cv::Mat scaledFrame;
    cv::resize(frame, scaledFrame,
               cv::Size(videoDisplayWidth, videoDisplayHeight));
    scaledFrame.copyTo(displayFrame(videoRect));
  }
}

void UIModule::drawControls(const MediaStats &stats, double duration) {
  int controlsY = videoDisplayHeight;

  // Draw timeline
  drawTimeline(stats.currentTime, duration, controlsY + 10);

  // Draw play button and volume slider below timeline
  int belowTimelineY = controlsY + config.timelineHeight + 5;

  // Play/Pause button
  cv::Rect playBtn(10, belowTimelineY, 80, 30);
  cv::rectangle(displayFrame, playBtn, config.buttonColor, -1);
  cv::rectangle(displayFrame, playBtn, config.textColor, 1);
  std::string playText = (stats.state == MediaState::PLAYING) ? "Pause" : "Play";
  cv::putText(displayFrame, playText, cv::Point(playBtn.x + 15, belowTimelineY + 20),
              cv::FONT_HERSHEY_SIMPLEX, 0.4, config.textColor, 1);

  // Draw volume slider next to play button
  float volume =
      (stats.audioStats.isPlaying || stats.state != MediaState::STOPPED)
          ? 0.7f
          : 0.7f; // Default volume
  drawVolumeSlider(volume, belowTimelineY);

  // Draw right panel with controls
  drawRightPanel(stats.state == MediaState::PLAYING);

  // Draw stats overlaid on video area (upper left)
  drawStats(stats, 10);

  // Draw CSV info overlaid on video area (below stats)
  drawCSVInfo(60);
}

void UIModule::drawTimeline(double currentTime, double duration, int y) {
  cv::Rect timelineRect = getTimelineRect();
  timelineRect.y = y;

  // Timeline background
  cv::rectangle(displayFrame, timelineRect, config.timelineColor, -1);
  cv::rectangle(displayFrame, timelineRect, config.textColor, 1);

  if (duration > 0) {
    // Progress bar
    double progress = currentTime / duration;
    if (progress > 1.0)
      progress = 1.0;
    if (progress < 0.0)
      progress = 0.0;

    int progressWidth = static_cast<int>(timelineRect.width * progress);
    cv::Rect progressRect = timelineRect;
    progressRect.width = progressWidth;
    cv::rectangle(displayFrame, progressRect, config.progressColor, -1);

    // Time text
    std::string timeText =
        formatTime(currentTime) + " / " + formatTime(duration);
    cv::putText(displayFrame, timeText, cv::Point(timelineRect.x + 5, y + 15),
                cv::FONT_HERSHEY_SIMPLEX, 0.4, config.textColor, 1);
  }
}

void UIModule::drawRightPanel(bool isPlaying) {
  cv::Rect panelRect = getRightPanelRect();

  // Draw panel background
  cv::rectangle(displayFrame, panelRect, cv::Scalar(40, 40, 40), -1);
  cv::rectangle(displayFrame, panelRect, config.textColor, 2);

  int yPos = 10;
  int buttonWidth = config.rightPanelWidth - 20;
  int buttonHeight = 30;
  int xStart = videoDisplayWidth + 10;

  // Helper lambda to draw a button
  auto drawButton = [&](const std::string& text, cv::Rect rect, bool enabled = true) {
    cv::Scalar btnColor = enabled ? config.buttonColor : cv::Scalar(40, 40, 40);
    cv::rectangle(displayFrame, rect, btnColor, -1);
    cv::rectangle(displayFrame, rect, config.textColor, 1);

    int textBaseline;
    cv::Size textSize = cv::getTextSize(text, cv::FONT_HERSHEY_SIMPLEX, 0.35, 1, &textBaseline);
    cv::Point textOrg(rect.x + (rect.width - textSize.width) / 2,
                      rect.y + (rect.height + textSize.height) / 2 - 2);
    cv::putText(displayFrame, text, textOrg, cv::FONT_HERSHEY_SIMPLEX, 0.35, config.textColor, 1);
  };

  // Load Video button
  cv::Rect loadVideoBtn = getLoadVideoButtonRect();
  drawButton("Load Video", loadVideoBtn);
  yPos = loadVideoBtn.y + loadVideoBtn.height + 10;

  // Play/Pause button
  cv::Rect playBtn = getPlayButtonRect();
  drawButton(isPlaying ? "Pause" : "Play", playBtn);
  yPos = playBtn.y + playBtn.height + 10;

  // Load CSV button
  cv::Rect loadCSVBtn = getLoadCSVButtonRect();
  drawButton("Load CSV", loadCSVBtn);
  yPos = loadCSVBtn.y + loadCSVBtn.height + 10;

  // CSV Navigation buttons
  bool canGoPrev = currentRowIndex > 0;
  bool canGoNext = currentRowIndex >= 0 && currentRowIndex < static_cast<int>(csvData.size()) - 1;

  cv::Rect prevBtn = getPrevRowButtonRect();
  drawButton("< Prev", prevBtn, canGoPrev);

  cv::Rect nextBtn = getNextRowButtonRect();
  drawButton("Next >", nextBtn, canGoNext);
  yPos = nextBtn.y + nextBtn.height + 10;

  // Show row count
  if (currentRowIndex >= 0 && currentRowIndex < static_cast<int>(csvData.size())) {
    std::stringstream ss;
    ss << "Row " << (currentRowIndex + 1) << "/" << csvData.size();
    cv::putText(displayFrame, ss.str(), cv::Point(xStart + 5, yPos + 15),
                cv::FONT_HERSHEY_SIMPLEX, 0.35, config.textColor, 1);
    yPos += 25;

    // Show current mark if exists
    const CSVRow& row = csvData[currentRowIndex];
    if (!row.mark.empty()) {
      std::string markText = "Mark: " + row.mark;
      cv::putText(displayFrame, markText, cv::Point(xStart + 5, yPos + 15),
                  cv::FONT_HERSHEY_SIMPLEX, 0.35, cv::Scalar(100, 255, 100), 1);
    }
    yPos += 30;
  }

  // Section label for marking
  cv::putText(displayFrame, "--- Mark As ---", cv::Point(xStart + 20, yPos + 15),
              cv::FONT_HERSHEY_SIMPLEX, 0.35, config.textColor, 1);
  yPos += 25;

  // Marking buttons
  bool hasCSV = currentRowIndex >= 0 && currentRowIndex < static_cast<int>(csvData.size());

  cv::Rect interestingBtn = getMarkInterestingButtonRect();
  drawButton("Interesting", interestingBtn, hasCSV);

  cv::Rect notInterestingBtn = getMarkNotInterestingButtonRect();
  drawButton("Not Interest.", notInterestingBtn, hasCSV);

  cv::Rect skipBtn = getMarkSkipButtonRect();
  drawButton("Skip", skipBtn, hasCSV);
}

void UIModule::drawVolumeSlider(float volume, int y) {
  // Position volume control next to play button
  int volumeX = 110;  // After the play button (10 + 80 + 20 spacing)
  cv::putText(displayFrame, "Vol:", cv::Point(volumeX, y + 20),
              cv::FONT_HERSHEY_SIMPLEX, 0.4, config.textColor, 1);

  // Volume slider
  cv::Rect sliderRect = getVolumeSliderRect();
  sliderRect.y = y + 10;
  cv::rectangle(displayFrame, sliderRect, config.timelineColor, -1);
  cv::rectangle(displayFrame, sliderRect, config.textColor, 1);

  // Volume handle
  int handleX = sliderRect.x + static_cast<int>(volume * sliderRect.width);
  cv::circle(displayFrame,
             cv::Point(handleX, sliderRect.y + sliderRect.height / 2), 6,
             config.sliderColor, -1);
  cv::circle(displayFrame,
             cv::Point(handleX, sliderRect.y + sliderRect.height / 2), 6,
             config.textColor, 1);

  // Volume percentage
  int volumePercent = static_cast<int>(volume * 100);
  cv::putText(displayFrame, std::to_string(volumePercent) + "%",
              cv::Point(sliderRect.x + sliderRect.width + 5, y + 20),
              cv::FONT_HERSHEY_SIMPLEX, 0.4, config.textColor, 1);
}

void UIModule::drawStats(const MediaStats &stats, int y) {
  if (stats.state == MediaState::STOPPED)
    return;

  // Draw semi-transparent background for readability
  cv::Rect bgRect(5, y - 5, 400, 40);
  cv::Mat roi = displayFrame(bgRect);
  cv::Mat overlay = roi.clone();
  overlay.setTo(cv::Scalar(0, 0, 0));
  cv::addWeighted(overlay, 0.5, roi, 0.5, 0, roi);

  // Draw playback stats on video area (upper left)
  std::stringstream ss;
  ss << "Queue: " << stats.videoStats.queueSize
     << " | Dropped: " << stats.videoStats.droppedFrames;
  cv::putText(displayFrame, ss.str(), cv::Point(10, y + 5),
              cv::FONT_HERSHEY_SIMPLEX, 0.4, cv::Scalar(255, 255, 255), 1);

  ss.str("");
  ss << "Audio: " << static_cast<int>(stats.audioStats.bufferFullness * 100) << "%";
  if (stats.videoStats.hardwareAccelEnabled) {
    ss << " | HW: " << stats.videoStats.hwAccelType;
  }
  cv::putText(displayFrame, ss.str(), cv::Point(10, y + 25),
              cv::FONT_HERSHEY_SIMPLEX, 0.4, cv::Scalar(255, 255, 255), 1);
}

void UIModule::drawCSVInfo(int y) {
  if (currentRowIndex < 0 || currentRowIndex >= static_cast<int>(csvData.size()))
    return;

  const CSVRow& row = csvData[currentRowIndex];

  // Draw semi-transparent background
  cv::Rect bgRect(5, y - 5, 450, 70);
  cv::Mat roi = displayFrame(bgRect);
  cv::Mat overlay = roi.clone();
  overlay.setTo(cv::Scalar(0, 0, 0));
  cv::addWeighted(overlay, 0.5, roi, 0.5, 0, roi);

  // Draw CSV row info
  std::stringstream ss;
  ss << "CSV Row " << (currentRowIndex + 1) << "/" << csvData.size()
     << " | Rank: " << row.rank;
  cv::putText(displayFrame, ss.str(), cv::Point(10, y + 5),
              cv::FONT_HERSHEY_SIMPLEX, 0.4, cv::Scalar(100, 255, 255), 1);

  ss.str("");
  ss << "Type: " << row.type << " | Class: " << row.clippability_class;
  cv::putText(displayFrame, ss.str(), cv::Point(10, y + 25),
              cv::FONT_HERSHEY_SIMPLEX, 0.4, cv::Scalar(100, 255, 255), 1);

  ss.str("");
  ss << "Score: " << std::fixed << std::setprecision(2) << row.clippability_score;
  ss << " | Time: " << row.start_time << "-" << row.end_time
     << "s (" << row.duration << "s)";
  cv::putText(displayFrame, ss.str(), cv::Point(10, y + 45),
              cv::FONT_HERSHEY_SIMPLEX, 0.4, cv::Scalar(100, 255, 255), 1);
}

void UIModule::onMouseClick(int event, int x, int y, int /*flags*/, void* userdata) {
    UIModule* ui = static_cast<UIModule*>(userdata);
    if (event == cv::EVENT_LBUTTONDOWN) {
        ui->handleMouseClick(x, y);
    }
}

void UIModule::handleMouseClick(int x, int y) {
  // Timeline click
  cv::Rect timelineRect = getTimelineRect();
  timelineRect.y = videoDisplayHeight + 10;

  if (isPointInRect(x, y, timelineRect)) {
    double clickRatio =
        static_cast<double>(x - timelineRect.x) / timelineRect.width;
    if (callbacks.onSeek) {
      callbacks.onSeek(clickRatio);
    }
    return;
  }

  // Play button click (below timeline)
  cv::Rect playBtnBelow(10, videoDisplayHeight + config.timelineHeight + 5, 80, 30);
  if (isPointInRect(x, y, playBtnBelow)) {
    if (callbacks.onTogglePlayPause) callbacks.onTogglePlayPause();
    return;
  }

  // Volume slider click (below timeline, next to play button)
  cv::Rect volumeSlider = getVolumeSliderRect();
  volumeSlider.y = videoDisplayHeight + config.timelineHeight + 15;

  if (isPointInRect(x, y, volumeSlider)) {
    float newVolume =
        static_cast<float>(x - volumeSlider.x) / volumeSlider.width;
    if (newVolume < 0.0f)
      newVolume = 0.0f;
    if (newVolume > 1.0f)
      newVolume = 1.0f;

    if (callbacks.onVolumeChange) {
      callbacks.onVolumeChange(newVolume);
    }
    return;
  }

  // Right panel buttons
  if (isPointInRect(x, y, getLoadVideoButtonRect())) {
    if (callbacks.onLoadVideo) callbacks.onLoadVideo();
    return;
  }

  if (isPointInRect(x, y, getPlayButtonRect())) {
    if (callbacks.onTogglePlayPause) callbacks.onTogglePlayPause();
    return;
  }

  if (isPointInRect(x, y, getLoadCSVButtonRect())) {
    if (callbacks.onLoadCSV) callbacks.onLoadCSV();
    return;
  }

  if (isPointInRect(x, y, getPrevRowButtonRect())) {
    if (callbacks.onPrevRow && currentRowIndex > 0) callbacks.onPrevRow();
    return;
  }

  if (isPointInRect(x, y, getNextRowButtonRect())) {
    if (callbacks.onNextRow && currentRowIndex >= 0 &&
        currentRowIndex < static_cast<int>(csvData.size()) - 1) {
      callbacks.onNextRow();
    }
    return;
  }

  // Marking buttons
  bool hasCSV = currentRowIndex >= 0 && currentRowIndex < static_cast<int>(csvData.size());

  if (hasCSV && isPointInRect(x, y, getMarkInterestingButtonRect())) {
    if (callbacks.onMarkRow) callbacks.onMarkRow("interesting");
    return;
  }

  if (hasCSV && isPointInRect(x, y, getMarkNotInterestingButtonRect())) {
    if (callbacks.onMarkRow) callbacks.onMarkRow("not_interesting");
    return;
  }

  if (hasCSV && isPointInRect(x, y, getMarkSkipButtonRect())) {
    if (callbacks.onMarkRow) callbacks.onMarkRow("skip");
    return;
  }
}

bool UIModule::isPointInRect(int x, int y, const cv::Rect &rect) const {
  return x >= rect.x && x <= (rect.x + rect.width) && y >= rect.y &&
         y <= (rect.y + rect.height);
}

int UIModule::processKeyboard(int timeoutMs) {
  int key = cv::waitKey(timeoutMs) & 0xFF;

  if (key == 27) { // ESC
    shouldExit = true;
    if (callbacks.onExit) {
      callbacks.onExit();
    }
  } else if (key == 32) { // Spacebar - Play/Pause
    if (callbacks.onTogglePlayPause) {
      callbacks.onTogglePlayPause();
    }
  } else if (key == 82 || key == 'w' || key == 'W') { // Up arrow or W - Volume up
    if (callbacks.onVolumeChangeRelative) {
      callbacks.onVolumeChangeRelative(0.05f);
    }
  } else if (key == 84 || key == 's' || key == 'S') { // Down arrow or S - Volume down
    if (callbacks.onVolumeChangeRelative) {
      callbacks.onVolumeChangeRelative(-0.05f);
    }
  } else if (key == 83 || key == 'd' || key == 'D') { // Right arrow or D - Seek forward 5 seconds
    if (callbacks.onSeekRelative) {
      callbacks.onSeekRelative(5.0);
    }
  } else if (key == 81 || key == 'a' || key == 'A') { // Left arrow or A - Seek backward 5 seconds
    if (callbacks.onSeekRelative) {
      callbacks.onSeekRelative(-5.0);
    }
  }

  return key;
}

std::string UIModule::openFileDialog() {
#ifdef _WIN32
  OPENFILENAME ofn;
  char szFile[260] = {0};

  ZeroMemory(&ofn, sizeof(ofn));
  ofn.lStructSize = sizeof(ofn);
  ofn.lpstrFile = szFile;
  ofn.nMaxFile = sizeof(szFile);
  ofn.lpstrFilter =
      "Video Files\0*.mp4;*.avi;*.mov;*.mkv;*.wmv;*.flv;*.webm\0All "
      "Files\0*.*\0";
  ofn.nFilterIndex = 1;
  ofn.Flags = OFN_PATHMUSTEXIST | OFN_FILEMUSTEXIST;

  if (GetOpenFileName(&ofn)) {
    return std::string(szFile);
  }
#elif defined(__linux__)
  std::string command = "zenity --file-selection --title=\"Select Video File\" "
                        "--file-filter=\"Video files | *.mp4 *.avi *.mov *.mkv "
                        "*.wmv *.flv *.webm\" 2>/dev/null";

  FILE *pipe = popen(command.c_str(), "r");
  if (pipe) {
    char buffer[1024];
    if (fgets(buffer, sizeof(buffer), pipe) != nullptr) {
      std::string filename(buffer);
      if (!filename.empty() && filename.back() == '\n') {
        filename.pop_back();
      }
      pclose(pipe);
      return filename;
    }
    pclose(pipe);
  } else {
    std::cout << "Zenity not available. Please enter video file path: ";
    std::string filename;
    std::getline(std::cin, filename);
    return filename;
  }
#else
  std::cout << "Please enter video file path: ";
  std::string filename;
  std::getline(std::cin, filename);
  return filename;
#endif

  return "";
}

std::string UIModule::formatTime(double seconds) const {
  if (seconds < 0)
    seconds = 0;
  int mins = static_cast<int>(seconds) / 60;
  int secs = static_cast<int>(seconds) % 60;

  std::stringstream ss;
  ss << mins << ":" << std::setfill('0') << std::setw(2) << secs;
  return ss.str();
}

cv::Rect UIModule::getTimelineRect() const {
  return cv::Rect(10, 0, videoDisplayWidth - 20, 20);
}

cv::Rect UIModule::getVolumeSliderRect() const {
  return cv::Rect(145, 0, config.volumeSliderWidth, 10);  // After "Vol:" label
}

// Right panel button positions
cv::Rect UIModule::getRightPanelRect() const {
  return cv::Rect(videoDisplayWidth, 0, config.rightPanelWidth, totalDisplayHeight);
}

cv::Rect UIModule::getLoadVideoButtonRect() const {
  int xStart = videoDisplayWidth + 10;
  int buttonWidth = config.rightPanelWidth - 20;
  return cv::Rect(xStart, 10, buttonWidth, 30);
}

cv::Rect UIModule::getPlayButtonRect() const {
  int xStart = videoDisplayWidth + 10;
  int buttonWidth = config.rightPanelWidth - 20;
  return cv::Rect(xStart, 50, buttonWidth, 30);
}

cv::Rect UIModule::getLoadCSVButtonRect() const {
  int xStart = videoDisplayWidth + 10;
  int buttonWidth = config.rightPanelWidth - 20;
  return cv::Rect(xStart, 90, buttonWidth, 30);
}

cv::Rect UIModule::getPrevRowButtonRect() const {
  int xStart = videoDisplayWidth + 10;
  int buttonWidth = (config.rightPanelWidth - 30) / 2;
  return cv::Rect(xStart, 130, buttonWidth, 30);
}

cv::Rect UIModule::getNextRowButtonRect() const {
  int xStart = videoDisplayWidth + 10;
  int buttonWidth = (config.rightPanelWidth - 30) / 2;
  return cv::Rect(xStart + buttonWidth + 10, 130, buttonWidth, 30);
}

cv::Rect UIModule::getMarkInterestingButtonRect() const {
  int xStart = videoDisplayWidth + 10;
  int buttonWidth = config.rightPanelWidth - 20;
  return cv::Rect(xStart, 250, buttonWidth, 30);
}

cv::Rect UIModule::getMarkNotInterestingButtonRect() const {
  int xStart = videoDisplayWidth + 10;
  int buttonWidth = config.rightPanelWidth - 20;
  return cv::Rect(xStart, 290, buttonWidth, 30);
}

cv::Rect UIModule::getMarkSkipButtonRect() const {
  int xStart = videoDisplayWidth + 10;
  int buttonWidth = config.rightPanelWidth - 20;
  return cv::Rect(xStart, 330, buttonWidth, 30);
}

void UIModule::setWindowSize(int width, int height) {
  // Width and height are for the video area only
  videoDisplayWidth = width;
  videoDisplayHeight = height;
  totalDisplayHeight =
      videoDisplayHeight + config.timelineHeight + config.buttonHeight;

  if (windowCreated) {
    // Calculate total window size: video width + right panel width
    int totalWindowWidth = videoDisplayWidth + config.rightPanelWidth;
    cv::resizeWindow(windowName, totalWindowWidth, totalDisplayHeight);

    std::cout << "Resized window to: " << totalWindowWidth << "x" << totalDisplayHeight
              << " (video: " << videoDisplayWidth << "x" << videoDisplayHeight << ")" << std::endl;
  }
}

bool UIModule::isWindowOpen() const { return windowCreated && !shouldExit; }

bool UIModule::shouldExitApplication() const { return shouldExit; }

void UIModule::showMessage(const std::string &message) {
  std::cout << "UI Message: " << message << std::endl;
}

void UIModule::showError(const std::string &error) {
  std::cerr << "UI Error: " << error << std::endl;
}

void UIModule::shutdown() {
  if (windowCreated) {
    cv::destroyWindow(windowName);
    windowCreated = false;
  }
}

std::string UIModule::openCSVFileDialog() {
#ifdef _WIN32
  OPENFILENAME ofn;
  char szFile[260] = {0};

  ZeroMemory(&ofn, sizeof(ofn));
  ofn.lStructSize = sizeof(ofn);
  ofn.lpstrFile = szFile;
  ofn.nMaxFile = sizeof(szFile);
  ofn.lpstrFilter = "CSV Files\0*.csv\0All Files\0*.*\0";
  ofn.nFilterIndex = 1;
  ofn.Flags = OFN_PATHMUSTEXIST | OFN_FILEMUSTEXIST;

  if (GetOpenFileName(&ofn)) {
    return std::string(szFile);
  }
#elif defined(__linux__)
  std::string command = "zenity --file-selection --title=\"Select CSV File\" "
                        "--file-filter=\"CSV files | *.csv\" 2>/dev/null";

  FILE *pipe = popen(command.c_str(), "r");
  if (pipe) {
    char buffer[1024];
    if (fgets(buffer, sizeof(buffer), pipe) != nullptr) {
      std::string filename(buffer);
      if (!filename.empty() && filename.back() == '\n') {
        filename.pop_back();
      }
      pclose(pipe);
      return filename;
    }
    pclose(pipe);
  } else {
    std::cout << "Zenity not available. Please enter CSV file path: ";
    std::string filename;
    std::getline(std::cin, filename);
    return filename;
  }
#else
  std::cout << "Please enter CSV file path: ";
  std::string filename;
  std::getline(std::cin, filename);
  return filename;
#endif

  return "";
}

bool UIModule::parseCSVFile(const std::string& filepath) {
  std::ifstream file(filepath);
  if (!file.is_open()) {
    std::cerr << "Failed to open CSV file: " << filepath << std::endl;
    return false;
  }

  csvData.clear();
  currentRowIndex = -1;
  csvFilePath = filepath;

  std::string line;
  bool isHeader = true;
  int rankCol = -1, typeCol = -1, classCol = -1, scoreCol = -1;
  int startCol = -1, endCol = -1, durationCol = -1, markCol = -1;

  while (std::getline(file, line)) {
    if (line.empty()) continue;

    std::vector<std::string> tokens;
    std::stringstream ss(line);
    std::string token;

    while (std::getline(ss, token, ',')) {
      // Trim whitespace
      size_t start = token.find_first_not_of(" \t\r\n");
      size_t end = token.find_last_not_of(" \t\r\n");
      if (start != std::string::npos) {
        token = token.substr(start, end - start + 1);
      } else {
        token = "";
      }
      tokens.push_back(token);
    }

    if (isHeader) {
      // Parse header to find column indices
      for (size_t i = 0; i < tokens.size(); i++) {
        if (tokens[i] == "rank") rankCol = i;
        else if (tokens[i] == "type") typeCol = i;
        else if (tokens[i] == "clippability_class") classCol = i;
        else if (tokens[i] == "clippability_score") scoreCol = i;
        else if (tokens[i] == "start_time") startCol = i;
        else if (tokens[i] == "end_time") endCol = i;
        else if (tokens[i] == "duration") durationCol = i;
        else if (tokens[i] == "mark") markCol = i;
      }
      isHeader = false;
    } else {
      // Parse data row
      CSVRow row;

      try {
        if (rankCol >= 0 && rankCol < static_cast<int>(tokens.size()) && !tokens[rankCol].empty()) {
          row.rank = std::stoi(tokens[rankCol]);
        }
      } catch (...) {
        row.rank = 0;
      }

      if (typeCol >= 0 && typeCol < static_cast<int>(tokens.size())) {
        row.type = tokens[typeCol];
      }

      if (classCol >= 0 && classCol < static_cast<int>(tokens.size())) {
        row.clippability_class = tokens[classCol];
      }

      try {
        if (scoreCol >= 0 && scoreCol < static_cast<int>(tokens.size()) && !tokens[scoreCol].empty()) {
          row.clippability_score = std::stod(tokens[scoreCol]);
        }
      } catch (...) {
        row.clippability_score = 0.0;
      }

      try {
        if (startCol >= 0 && startCol < static_cast<int>(tokens.size()) && !tokens[startCol].empty()) {
          row.start_time = std::stod(tokens[startCol]);
        }
      } catch (...) {
        row.start_time = 0.0;
      }

      try {
        if (endCol >= 0 && endCol < static_cast<int>(tokens.size()) && !tokens[endCol].empty()) {
          row.end_time = std::stod(tokens[endCol]);
        }
      } catch (...) {
        row.end_time = 0.0;
      }

      try {
        if (durationCol >= 0 && durationCol < static_cast<int>(tokens.size()) && !tokens[durationCol].empty()) {
          row.duration = std::stod(tokens[durationCol]);
        }
      } catch (...) {
        row.duration = 0.0;
      }

      // Read mark column if exists
      if (markCol >= 0 && markCol < static_cast<int>(tokens.size())) {
        row.mark = tokens[markCol];
      }

      csvData.push_back(row);
    }
  }

  file.close();

  if (!csvData.empty()) {
    currentRowIndex = 0;
    std::cout << "Loaded " << csvData.size() << " rows from CSV" << std::endl;
    return true;
  }

  return false;
}

void UIModule::navigateToRow(int direction) {
  if (csvData.empty()) return;

  int newIndex = currentRowIndex + direction;
  if (newIndex >= 0 && newIndex < static_cast<int>(csvData.size())) {
    currentRowIndex = newIndex;
  }
}

const CSVRow* UIModule::getCurrentRow() const {
  if (currentRowIndex >= 0 && currentRowIndex < static_cast<int>(csvData.size())) {
    return &csvData[currentRowIndex];
  }
  return nullptr;
}

int UIModule::getCurrentRowIndex() const {
  return currentRowIndex;
}

int UIModule::getTotalRows() const {
  return static_cast<int>(csvData.size());
}

void UIModule::markCurrentRow(const std::string& mark) {
  if (currentRowIndex >= 0 && currentRowIndex < static_cast<int>(csvData.size())) {
    csvData[currentRowIndex].mark = mark;
    saveCSVMarks();
    std::cout << "Marked row " << (currentRowIndex + 1) << " as '" << mark << "'" << std::endl;
  }
}

void UIModule::saveCSVMarks() {
  if (csvFilePath.empty() || csvData.empty()) {
    return;
  }

  std::ofstream file(csvFilePath);
  if (!file.is_open()) {
    std::cerr << "Failed to save CSV marks to: " << csvFilePath << std::endl;
    return;
  }

  // Write header
  file << "rank,type,clippability_class,clippability_score,start_time,end_time,duration,mark\n";

  // Write data rows
  for (const auto& row : csvData) {
    file << row.rank << ","
         << row.type << ","
         << row.clippability_class << ","
         << row.clippability_score << ","
         << row.start_time << ","
         << row.end_time << ","
         << row.duration << ","
         << row.mark << "\n";
  }

  file.close();
  std::cout << "Saved CSV marks to: " << csvFilePath << std::endl;
}