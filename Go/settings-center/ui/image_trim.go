// Image decoding leaves the runtime holding large short-lived spans (a 3000×3000
// cover is a 36 MB buffer, and a wallpaper is another). They are garbage at once
// but the scavenger returns them only minutes later, which is what users see as
// the control center's memory. One trim, debounced, after the picture is in its
// bitmap gives those pages back without touching the frame being drawn.
package ui

import (
	"runtime/debug"
	"sync/atomic"
	"time"
)

var trimPending atomic.Bool

// trimDecodedImageMemory asks the runtime to return the free heap pages a decode
// left. It never blocks the caller: the collector runs once the frame that loaded
// the picture is done, and a burst of loads shares one trim.
func trimDecodedImageMemory() {
	if !trimPending.CompareAndSwap(false, true) {
		return
	}
	go func() {
		time.Sleep(300 * time.Millisecond)
		debug.FreeOSMemory()
		trimPending.Store(false)
	}()
}
