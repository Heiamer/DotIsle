// The Icon set of the Vello app mapped to its Material Symbols file names
// (dotisle-render/src/material_symbols.rs icon_name).
package ui

func iconFileName(kind uint8) string {
	return IconNames[kind]
}

// Icon ids, mirroring dotisle_render::draw::Icon order.
const (
	IconHome = iota
	IconMusic
	IconFocus
	IconTodo
	IconSettings
	IconPlay
	IconPause
	IconNext
	IconPrev
	IconPlus
	IconCheck
	IconClose
	IconArrow
	IconIsland
	IconSun
	IconMoon
	IconBack
	IconTrash
	IconReset
	IconSearch
	IconVolume
	IconFolder
	IconImport
	IconLeaf
	IconStop
	IconMinimize
	IconMaximize
	IconRestore
	IconTranslate
	IconHeart
	IconMove
	IconCount
)

var IconNames = [IconCount]string{
	"home", "music_note", "timer", "checklist", "settings",
	"play_arrow", "pause", "skip_next", "skip_previous",
	"add", "check", "close", "arrow_forward", "dialpad",
	"light_mode", "dark_mode", "arrow_back", "delete", "restart_alt",
	"search", "volume_up", "folder", "download", "eco", "stop",
	"minimize", "crop_square", "filter_none", "translate", "favorite", "open_with",
}
