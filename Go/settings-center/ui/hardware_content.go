package ui

import (
	"errors"
	"time"

	mygoui "github.com/egoist/mygo/ui"
)

type hardwareRenderer interface {
	RequireHardwareRendering(func(error), func(time.Time))
}

// HasHardwareRenderer reports whether the production MyGo patch was linked.
func HasHardwareRenderer() bool {
	_, ok := any((*mygoui.Content)(nil)).(hardwareRenderer)
	return ok
}

// HardwareContent leaves unpatched development builds available for unit tests, but
// refuses to open the production window when the hardware policy is unavailable.
func HardwareContent(view func(*mygoui.Context), onFailure func(error), onPresent func(time.Time)) (*mygoui.Content, error) {
	content := mygoui.View(view)
	if renderer, ok := any(content).(hardwareRenderer); ok {
		renderer.RequireHardwareRendering(onFailure, onPresent)
		return content, nil
	}
	return nil, errors.New("build.cmd must prepare the hardware-only renderer")
}
