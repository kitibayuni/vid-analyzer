#include "ui_module.h"
#include "media_controller.h"
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
      videoDisplayWidth(800), videoDisplayHeight(600),  // Move these up
      windowCreated(false), shouldExit(false) {         // Move these down
}

UIModule::~UIModule() { shutdown(); }

bool UIModule::initialize(const UIConfig &cfg) {
  std::cout << "Initializing UIModule..." << std::endl;

  config = cfg;
  videoDisplayWidth = config.windowWidth;
  videoDisplayHeight = config.windowHeight;
  totalDisplayHeight =
      videoDisplayHeight + config.timelineHeight + config.buttonHeight;

  // Create OpenCV window
  cv::namedWindow(windowName, cv::WINDOW_AUTOSIZE);
  cv::setMouseCallback(windowName, onMouseClick, this);
  windowCreated = true;

  createInitialDisplay();

  std::cout << "UIModule initialized successfully" << std::endl;
  std::cout << "Window size: " << videoDisplayWidth << "x" << totalDisplayHeight
            << std::endl;

  return true;
}

void UIModule::setCallbacks(const UICallbacks &cb) { callbacks = cb; }

void UIModule::createInitialDisplay() {
  displayFrame = cv::Mat::zeros(totalDisplayHeight, videoDisplayWidth, CV_8UC3);
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

  // Create new display frame
  displayFrame = cv::Mat::zeros(totalDisplayHeight, videoDisplayWidth, CV_8UC3);

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

  // Draw buttons
  int buttonsY = controlsY + config.timelineHeight;
  drawButtons(stats.state == MediaState::PLAYING, buttonsY + 5);

  // Draw volume slider
  float volume =
      (stats.audioStats.isPlaying || stats.state != MediaState::STOPPED)
          ? 0.7f
          : 0.7f; // Default volume
  drawVolumeSlider(volume, buttonsY + 5);

  // Draw stats
  drawStats(stats, buttonsY + 5);
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

void UIModule::drawButtons(bool isPlaying, int y) {
  // Load button
  cv::Rect loadButton = getLoadButtonRect();
  loadButton.y = y;
  cv::rectangle(displayFrame, loadButton, config.buttonColor, -1);
  cv::rectangle(displayFrame, loadButton, config.textColor, 1);
  cv::putText(displayFrame, "Load Video", cv::Point(loadButton.x + 5, y + 18),
              cv::FONT_HERSHEY_SIMPLEX, 0.4, config.textColor, 1);

  // Play/Pause button
  cv::Rect playButton = getPlayButtonRect();
  playButton.y = y;
  cv::rectangle(displayFrame, playButton, config.buttonColor, -1);
  cv::rectangle(displayFrame, playButton, config.textColor, 1);
  std::string playText = isPlaying ? "Pause" : "Play";
  cv::putText(displayFrame, playText, cv::Point(playButton.x + 20, y + 18),
              cv::FONT_HERSHEY_SIMPLEX, 0.4, config.textColor, 1);
}

void UIModule::drawVolumeSlider(float volume, int y) {
  int volumeX = 200;
  cv::putText(displayFrame, "Volume:", cv::Point(volumeX, y + 18),
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
              cv::Point(sliderRect.x + sliderRect.width + 10, y + 18),
              cv::FONT_HERSHEY_SIMPLEX, 0.4, config.textColor, 1);
}

void UIModule::drawStats(const MediaStats &stats, int y) {
  if (stats.state == MediaState::STOPPED)
    return;

  std::stringstream ss;
  ss << "Queue: " << stats.videoStats.queueSize
     << " | Dropped: " << stats.videoStats.droppedFrames
     << " | Audio: " << static_cast<int>(stats.audioStats.bufferFullness * 100)
     << "%";

  if (stats.videoStats.hardwareAccelEnabled) {
    ss << " | HW: " << stats.videoStats.hwAccelType;
  }

  cv::putText(displayFrame, ss.str(), cv::Point(400, y + 18),
              cv::FONT_HERSHEY_SIMPLEX, 0.3, cv::Scalar(180, 180, 180), 1);
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
      callbacks.onSeek(
          clickRatio); // Pass ratio, let controller calculate actual time
    }
    return;
  }

  // Load button click
  cv::Rect loadButton = getLoadButtonRect();
  loadButton.y = videoDisplayHeight + config.timelineHeight + 5;

  if (isPointInRect(x, y, loadButton)) {
    if (callbacks.onLoadVideo) {
      callbacks.onLoadVideo();
    }
    return;
  }

  // Play button click
  cv::Rect playButton = getPlayButtonRect();
  playButton.y = videoDisplayHeight + config.timelineHeight + 5;

  if (isPointInRect(x, y, playButton)) {
    if (callbacks.onTogglePlayPause) {
      callbacks.onTogglePlayPause();
    }
    return;
  }

  // Volume slider click
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

cv::Rect UIModule::getLoadButtonRect() const { return cv::Rect(10, 0, 80, 30); }

cv::Rect UIModule::getPlayButtonRect() const {
  return cv::Rect(100, 0, 80, 30);
}

cv::Rect UIModule::getVolumeSliderRect() const {
  return cv::Rect(250, 0, config.volumeSliderWidth, 10);
}

void UIModule::setWindowSize(int width, int height) {
  videoDisplayWidth = width;
  videoDisplayHeight = height;
  totalDisplayHeight =
      videoDisplayHeight + config.timelineHeight + config.buttonHeight;

  if (windowCreated) {
    cv::resizeWindow(windowName, videoDisplayWidth, totalDisplayHeight);
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