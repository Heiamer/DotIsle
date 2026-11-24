package main

import (
	"fmt"
	"log"
	"runtime"
	"runtime/debug"
	"time"
	"unsafe"

	"dotisle/settings/ui"
	"github.com/egoist/mygo"
	"golang.org/x/sys/windows"
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
	gpuAlert           bool
}

func (c *centerWindow) show() {
	if c.idleExit || c.gpuAlert {
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
	content, err := ui.HardwareContent(c.app.View, c.hardwareFailure, nil)
	if err != nil {
		c.hardwareFailure(err)
		return
	}
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
		Content:           content,
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

// A lost device is retried by MyGo once when possible. If hardware still
// cannot present, close only the control center, not the island or widgets.
func (c *centerWindow) hardwareFailure(err error) {
	log.Printf("control center hardware rendering: %v", err)
	c.gpuAlert = true
	if win := c.app.Win; win != nil {
		c.cancelRelease()
		c.app.EndCenterVisit()
		c.app.DetachCenterWindow()
		win.Destroy()
	}
	c.ready, c.released = false, true
	title, _ := windows.UTF16PtrFromString("点屿 · 控制中心")
	text, _ := windows.UTF16PtrFromString(fmt.Sprintf("D3D11 硬件渲染不可用，已关闭控制中心，未切换到 CPU 渲染。\n\n请检查显卡驱动，再重新打开控制中心。\n\n%v", err))
	const errorDialog = 0x00010010 // MB_SETFOREGROUND | MB_ICONERROR
	windows.NewLazySystemDLL("user32.dll").NewProc("MessageBoxW").Call(0, uintptr(unsafe.Pointer(text)), uintptr(unsafe.Pointer(title)), errorDialog)
	runtime.KeepAlive(text)
	runtime.KeepAlive(title)
	c.gpuAlert = false
	c.tryIdleExit()
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
