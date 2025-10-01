#ifndef UI_MODULE_H
#define UI_MODULE_H

#include <functional>
#include <opencv2/opencv.hpp>
#include <string>
#include <vector>

// Forward declaration
class MediaController;
struct MediaStats;

// CSV row data structure
struct CSVRow {
  int rank = 0;
  std::string type;
  std::string clippability_class;
  double clippability_score = 0.0;
  double start_time = 0.0;
  double end_time = 0.0;
  double duration = 0.0;
  std::string mark;  // "interesting", "not_interesting", "skip", or empty
};

struct UIConfig {
  int windowWidth = 1000;  // Increased to accommodate right panel
  int windowHeight = 600;
  int rightPanelWidth = 200;  // Width of control panel on right
  int timelineHeight = 40;
  int buttonHeight = 40;
  int volumeSliderWidth = 100;

  // Colors (BGR format for OpenCV)
  cv::Scalar bgColor = cv::Scalar(50, 50, 50);
  cv::Scalar timelineColor = cv::Scalar(100, 100, 100);
  cv::Scalar progressColor = cv::Scalar(255, 150, 0);  // Blue (BGR format)
  cv::Scalar buttonColor = cv::Scalar(70, 70, 70);
  cv::Scalar textColor = cv::Scalar(255, 255, 255);
  cv::Scalar sliderColor = cv::Scalar(255, 150, 0);

  UIConfig() = default;
};

// UI event callbacks - allows UI to communicate back to application
struct UICallbacks {
  std::function<void()> onLoadVideo;
  std::function<void()> onTogglePlayPause;
  std::function<void(double)> onSeek;  // For timeline clicks (ratio 0-1)
  std::function<void(double)> onSeekRelative;  // For arrow keys (seconds delta)
  std::function<void(float)> onVolumeChange;
  std::function<void(float)> onVolumeChangeRelative;  // For arrow keys (delta)
  std::function<void()> onExit;
  std::function<void()> onLoadCSV;
  std::function<void()> onNextRow;
  std::function<void()> onPrevRow;
  std::function<void(const std::string&)> onMarkRow;  // Mark current row and advance

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

  // CSV data
  std::vector<CSVRow> csvData;
  int currentRowIndex;
  std::string csvFilePath;  // Store path for saving marks

  // File dialog implementation varies by platform
  std::string openFileDialog();
  std::string openCSVFileDialog();

  // Drawing methods
  void createInitialDisplay();
  void drawControls(const MediaStats &stats, double duration);
  void drawTimeline(double currentTime, double duration, int y);
  void drawRightPanel(bool isPlaying);
  void drawVolumeSlider(float volume, int y);
  void drawStats(const MediaStats &stats, int y);
  void drawCSVInfo(int y);
  void drawVideoFrame(const cv::Mat &frame);

  // Event handling
  static void onMouseClick(int event, int x, int y, int flags, void *userdata);
  void handleMouseClick(int x, int y);
  bool isPointInRect(int x, int y, const cv::Rect &rect) const;

  // Utility methods
  std::string formatTime(double seconds) const;
  cv::Rect getTimelineRect() const;
  cv::Rect getVolumeSliderRect() const;

  // Right panel button rects
  cv::Rect getRightPanelRect() const;
  cv::Rect getLoadVideoButtonRect() const;
  cv::Rect getLoadCSVButtonRect() const;
  cv::Rect getPlayButtonRect() const;
  cv::Rect getPrevRowButtonRect() const;
  cv::Rect getNextRowButtonRect() const;
  cv::Rect getMarkInterestingButtonRect() const;
  cv::Rect getMarkNotInterestingButtonRect() const;
  cv::Rect getMarkSkipButtonRect() const;

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

  // CSV access
  const CSVRow* getCurrentRow() const;
  int getCurrentRowIndex() const;
  int getTotalRows() const;
  bool parseCSVFile(const std::string& filepath);
  void navigateToRow(int direction);
  void markCurrentRow(const std::string& mark);
  void saveCSVMarks();

  // Disable copy/assignment
  UIModule(const UIModule &) = delete;
  UIModule &operator=(const UIModule &) = delete;
  UIModule(UIModule &&) = delete;
  UIModule &operator=(UIModule &&) = delete;
};

#endif // UI_MODULE_H