# Load libraries
if(!require(ggplot2)) install.packages("ggplot2")
if(!require(dplyr)) install.packages("dplyr")
if(!require(tidyr)) install.packages("tidyr")
if(!require(zoo)) install.packages("zoo") # for NA interpolation
if(!require(gridExtra)) install.packages("gridExtra")

library(ggplot2)
library(dplyr)
library(tidyr)
library(zoo)
library(gridExtra)

# -------------------------
# Parameters: columns to plot
# -------------------------
y_columns <- c("total_engag_1s_pct", "total_engag_10s_pct")

# -------------------------
# Load CSVs
# -------------------------
new <- read.csv("C:/Users/Administrator/Code/vid-analyzer/temp/total_engagement.csv",
                header = TRUE, stringsAsFactors = FALSE)
original <- read.csv("C:/Users/Administrator/Code/vid-analyzer/temp/quicky_audio0_vocals_merged.csv",
                     header = TRUE, stringsAsFactors = FALSE)

# -------------------------
# Clean column names
# -------------------------
clean_names <- function(df) {
  colnames(df) <- tolower(trimws(colnames(df)))
  return(df)
}
new <- clean_names(new)
original <- clean_names(original)

# -------------------------
# Ensure time_sec column exists
# -------------------------
if(!"time_sec" %in% colnames(new) & "time" %in% colnames(new)) {
  new <- new %>% rename(time_sec = time)
}
if(!"time_sec" %in% colnames(original) & "time" %in% colnames(original)) {
  original <- original %>% rename(time_sec = time)
}

# -------------------------
# Interpolate NAs in original for the selected columns
# -------------------------
for(col in y_columns) {
  if(col %in% colnames(original)) {
    original[[col]] <- na.approx(original[[col]], x = original$time_sec, na.rm = FALSE)
  }
}

# -------------------------
# Helper function to add geom_line if column exists
# -------------------------
add_geom_lines <- function(plot, df, columns) {
  for(col in columns) {
    if(col %in% colnames(df)) {
      plot <- plot + geom_line(aes_string(y = col, color = shQuote(col)), linewidth = 1)
    } else {
      message(paste("Column", col, "not found in dataset. Skipping."))
    }
  }
  return(plot)
}

# -------------------------
# Plot: New (top)
# -------------------------
plot_new <- ggplot(new, aes(x = time_sec)) +
  labs(title = "New - Spectral Engagement EMA",
       x = "Time (sec)", y = "Engagement (%)",
       color = "EMA Type") +
  theme_minimal()

plot_new <- add_geom_lines(plot_new, new, y_columns)

# -------------------------
# Plot: Original (bottom)
# -------------------------
plot_original <- ggplot(original, aes(x = time_sec)) +
  labs(title = "Original - Spectral Engagement EMA",
       x = "Time (sec)", y = "Engagement (%)",
       color = "EMA Type") +
  theme_minimal()

plot_original <- add_geom_lines(plot_original, original, y_columns)

# -------------------------
# Combine plots vertically
# -------------------------
grid.arrange(plot_new, plot_original, ncol = 1)
