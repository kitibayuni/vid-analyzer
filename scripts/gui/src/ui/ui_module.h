#ifndef UI_MODULE_H
#define UI_MODULE_H

#include <functional>
#include <opencv2/opencv.hpp>
#include <string>

// Forward declaration
class MediaController;
struct MediaStats;

struct UIConfig {
  int windowWidth = 800;
  int windowHeight = 600;
  int timelineHeight = 40;
  int buttonHeight = 40;
  int volumeSliderWidth = 100;

  // Colors (BGR format for OpenCV)
  cv::Scalar bgColor = cv::Scalar(50, 50, 50);
  cv::Scalar timelineColor = cv::Scalar(100, 100, 100);
  cv::Scalar progressColor = cv::Scalar(0, 255, 0);
  cv::Scalar buttonColor = cv::Scalar(70, 70, 70);
  cv::Scalar textColor = cv::Scalar(255, 255, 255);
  cv::Scalar sliderColor = cv::Scalar(255, 150, 0);

  UIConfig() = default;
};

// UI event callbacks - allows UI to communicate back to application
struct UICallbacks {
  std::function<void()> onLoadVideo;
  std::function<void()> onTogglePlayPause;
  std::function<void(double)> onSeek;
  std::function<void(float)> onVolumeChange;
  std::function<void()> onExit;

  UICallbacks() = default;
};

class UIModule {
private:
  // OpenCV window
  std::string windowName;
  cv::Mat displayFrame;
  UIConfig config;
  UICallbacks callbacks;

  // Layout dimensions
  int totalDisplayHeight;
  int videoDisplayWidth;
  int videoDisplayHeight;

  // State
  bool windowCreated;
  bool shouldExit;

  // File dialog implementation varies by platform
  std::string openFileDialog();

  // Drawing methods
  void createInitialDisplay();
  void drawControls(const MediaStats &stats, double duration);
  void drawTimeline(double currentTime, double duration, int y);
  void drawButtons(bool isPlaying, int y);
  void drawVolumeSlider(float volume, int y);
  void drawStats(const MediaStats &stats, int y);
  void drawVideoFrame(const cv::Mat &frame);

  // Event handling
  static void onMouseClick(int event, int x, int y, int flags, void *userdata);
  void handleMouseClick(int x, int y);
  bool isPointInRect(int x, int y, const cv::Rect &rect) const;

  // Utility methods
  std::string formatTime(double seconds) const;
  cv::Rect getTimelineRect() const;
  cv::Rect getLoadButtonRect() const;
  cv::Rect getPlayButtonRect() const;
  cv::Rect getVolumeSliderRect() const;

public:
  UIModule();
  ~UIModule();

  // Core functionality
  bool initialize(const UIConfig &cfg = UIConfig());
  void setCallbacks(const UICallbacks &cb);
  void shutdown();

  // Main update loop - should be called regularly from main thread
  bool update(const cv::Mat &videoFrame, const MediaStats &stats,
              double duration);

  // Window management
  bool isWindowOpen() const;
  void setWindowSize(int width, int height);

  // Display control
  void showMessage(const std::string &message);
  void showError(const std::string &error);

  // Input handling
  int processKeyboard(int timeoutMs = 1); // Returns key code, -1 if no key

  // Properties
  bool shouldExitApplication() const;

  // Disable copy/assignment
  UIModule(const UIModule &) = delete;
  UIModule &operator=(const UIModule &) = delete;
  UIModule(UIModule &&) = delete;
  UIModule &operator=(UIModule &&) = delete;
};

#endif // UI_MODULE_H