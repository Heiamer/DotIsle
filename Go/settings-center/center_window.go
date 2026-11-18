package main

import (
	"log"
	"runtime/debug"
	"time"

	"dotisle/settings/ui"
	"github.com/egoist/mygo"
	mygoui "github.com/egoist/mygo/ui"
)

const centerWindowGrace = 3 * time.Second

// centerWindow owns only the native window and its renderer, on the UI thread.
type centerWindow struct {
	app                *ui.App
	position           mygo.Rectangle
	hasPosition, ready bool
	hideGeneration     uint64
	release            *time.Timer
	allowIdleExit      bool
	idleExit           bool
	released           bool
}

func (c *centerWindow) show() {
	if c.idleExit {
		return
	}
	if win := c.app.Win; win != nil {
		if !c.ready {
			return // let the first native frame show the newly created window
		}
		if win.IsMinimized() {
			win.Restore()
		}
		win.Show()
		return
	}
	ui.RegisterFonts()
	startupMark("fonts")
	c.released = false
	c.app.BeginCenterVisit()
	options := mygo.WindowOptions{
		Title:             "点屿 · 控制中心",
		Width:             ui.WindowW,
		Height:            ui.WindowH,
		MinWidth:          ui.WindowW,
		MinHeight:         ui.WindowH,
		MaxWidth:          ui.WindowW,
		MaxHeight:         ui.WindowH,
		DisableResize:     true,
		DisableMaximize:   true,
		DisableFullScreen: true,
		Frameless:         true,
		Vibrancy:          mygo.VibrancyMica,
		BackgroundColor:   "transparent",
		Content:           mygoui.View(c.app.View),
	}
	if c.hasPosition {
		options.X, options.Y = c.position.X, c.position.Y
	}
	win := mygo.NewWindow(options)
	c.app.Win = win
	c.ready = false
	startupMark("window")
	win.OnReadyToShow(func() {
		c.ready = true
		startupMark("first-frame")
		if err := win.SetIcon(ui.AppIcon); err != nil {
			log.Printf("window icon: %v", err)
		}
	})
	win.OnClose(func(e *mygo.CloseEvent) {
		e.PreventDefault()
		win.Hide()
	})
	win.OnShow(func() {
		c.cancelRelease()
		c.app.BeginCenterVisit()
	})
	win.OnHide(func() {
		c.position, c.hasPosition = win.Bounds(), true
		c.app.EndCenterVisit()
		c.cancelRelease()
		generation := c.hideGeneration
		c.release = time.AfterFunc(centerWindowGrace, func() {
			mygo.RunOnMain(func() {
				if c.hideGeneration != generation || c.app.Win != win || win.IsVisible() {
					return
				}
				c.release = nil
				c.app.DetachCenterWindow()
				win.Destroy()
				c.released = true
				if !c.tryIdleExit() {
					go debug.FreeOSMemory()
				}
			})
		})
	})
}

func (c *centerWindow) tryIdleExit() bool {
	if c.allowIdleExit && c.released && !c.idleExit && c.app.CanIdleExit() {
		c.idleExit = true
		mygo.App.Quit()
	}
	return c.idleExit
}

func (c *centerWindow) cancelRelease() {
	c.hideGeneration++
	if c.release != nil {
		c.release.Stop()
		c.release = nil
	}
}
