package ui

import "time"

// calendarDateDistance counts local calendar dates rather than 24-hour spans.
// UTC is used only after extracting dates, so DST cannot change the day count.
func calendarDateDistance(raw string, now time.Time) (time.Time, int, error) {
	date, err := time.ParseInLocation("2006-01-02", raw, now.Location())
	if err != nil {
		return time.Time{}, 0, err
	}
	target := time.Date(date.Year(), date.Month(), date.Day(), 0, 0, 0, 0, time.UTC)
	today := time.Date(now.Year(), now.Month(), now.Day(), 0, 0, 0, 0, time.UTC)
	return date, int((target.Unix() - today.Unix()) / 86400), nil
}
