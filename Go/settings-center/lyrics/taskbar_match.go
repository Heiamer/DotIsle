// Port of H0308/taskbar-lyrics-displayer lyric_matching.cpp, Apache-2.0.
// Pinned upstream: 3fae9287dfb0636fc19a0df2587a1a59a54cf9bb.
package lyrics

import (
	"math"
	"sort"
	"strings"
	"syscall"
	"unicode/utf16"
	"unsafe"
)

var tbMap = syscall.NewLazyDLL("kernel32.dll").NewProc("LCMapStringEx")
var tbCRT = syscall.NewLazyDLL("msvcrt.dll")
var tbLowerProc = tbCRT.NewProc("towlower")
var tbSpaceProc = tbCRT.NewProc("iswspace")
var tbAlnumProc = tbCRT.NewProc("iswalnum")

func tbRune(n uintptr) rune {
	if n >= 0xd800 && n <= 0xdfff {
		return rune(0xf0000 + n - 0xd800)
	}
	return rune(n)
}
func tbUnit(r rune) uintptr {
	if r >= 0xf0000 && r <= 0xf07ff {
		return uintptr(r - 0xf0000 + 0xd800)
	}
	return uintptr(r)
}

// Represent Windows wchar_t code units, including surrogate pairs, verbatim.
func tbWide(s string) string {
	var r []rune
	for _, u := range utf16.Encode([]rune(s)) {
		r = append(r, tbRune(uintptr(u)))
	}
	return string(r)
}
func tbUTF8(s string) string {
	var u []uint16
	for _, r := range s {
		u = append(u, uint16(tbUnit(r)))
	}
	return string(utf16.Decode(u))
}
func tbTrim(s string) string { return strings.Trim(s, " \t\r\n") }
func tbLower(s string) string {
	return strings.Map(func(r rune) rune { n, _, _ := tbLowerProc.Call(tbUnit(r)); return tbRune(n) }, s)
}
func tbSpace(r rune) bool { n, _, _ := tbSpaceProc.Call(tbUnit(r)); return n != 0 }
func tbAlnum(r rune) bool { n, _, _ := tbAlnumProc.Call(tbUnit(r)); return n != 0 }
func tbSimple(s string) string {
	if s == "" {
		return s
	}
	var in []uint16
	for _, r := range s {
		in = append(in, uint16(tbUnit(r)))
	}
	locale, _ := syscall.UTF16PtrFromString("zh-CN")
	n, _, _ := tbMap.Call(uintptr(unsafe.Pointer(locale)), 0x02000000, uintptr(unsafe.Pointer(&in[0])), uintptr(len(in)), 0, 0, 0, 0, 0)
	if n == 0 {
		return s
	}
	out := make([]uint16, n)
	got, _, _ := tbMap.Call(uintptr(unsafe.Pointer(locale)), 0x02000000, uintptr(unsafe.Pointer(&in[0])), uintptr(len(in)), uintptr(unsafe.Pointer(&out[0])), n, 0, 0, 0)
	if got != n {
		return s
	}
	var r []rune
	for _, v := range out {
		r = append(r, tbRune(uintptr(v)))
	}
	return string(r)
}
func tbContains(raw, word string) bool {
	s, w := []rune(tbLower(tbSimple(raw))), []rune(tbLower(word))
	if len(w) == 0 {
		return false
	}
	ascii := true
	for _, r := range w {
		if r >= 128 || !tbAlnum(r) && r != '.' {
			ascii = false
		}
	}
	for i := 0; i+len(w) <= len(s); i++ {
		if string(s[i:i+len(w)]) == string(w) && (!ascii || i == 0 || !tbAlnum(s[i-1])) && (!ascii || i+len(w) == len(s) || !tbAlnum(s[i+len(w)])) {
			return true
		}
	}
	return false
}

var tbVersionWords = []struct {
	tag     string
	aliases []string
}{
	{"live", []string{"live", "现场", "演唱会", "concert"}}, {"remix", []string{"remix", "混音"}},
	{"acoustic", []string{"acoustic", "unplugged", "不插电"}}, {"instrumental", []string{"instrumental", "karaoke", "伴奏", "纯音乐"}},
	{"demo", []string{"demo", "小样"}}, {"simlish", []string{"simlish"}}, {"japanese", []string{"japanese", "japan", "日文", "日语", "日本語"}},
	{"english", []string{"english", "英文", "英语"}}, {"chinese", []string{"chinese", "中文", "中文版", "国语", "普通话"}},
	{"special", []string{"dj", "dj版", "剧情", "剧情版"}}, {"award", []string{"vma", "award", "颁奖", "典礼"}},
	{"remastered", []string{"remastered", "重制"}}, {"ai", []string{"ai翻唱", "ai 翻唱", "ai cover", "ai版", "ai声库"}}, {"cover", []string{"翻唱", "翻唱版", "cover"}},
}

func tbAppend(out []string, values ...string) []string {
	for _, v := range values {
		found := false
		for _, s := range out {
			if s == v {
				found = true
				break
			}
		}
		if !found {
			out = append(out, v)
		}
	}
	return out
}
func tbTagsText(s string) []string {
	var tags []string
	for _, g := range tbVersionWords {
		for _, a := range g.aliases {
			if tbContains(s, a) {
				tags = append(tags, g.tag)
				break
			}
		}
	}
	if len(tags) == 0 {
		for _, a := range []string{"version", "ver.", "edit", "special", "anniversary"} {
			if tbContains(s, a) {
				tags = append(tags, "version")
				break
			}
		}
	}
	return tags
}

var tbBrackets = map[rune]rune{'(': ')', '[': ']', '（': '）', '【': '】'}

func tbTags(s string) []string {
	var tags []string
	for _, pair := range []string{"()", "[]", "（）", "【】"} {
		p := []rune(pair)
		rest := s
		for {
			i := strings.IndexRune(rest, p[0])
			if i < 0 {
				break
			}
			rest = rest[i+len(string(p[0])):]
			j := strings.IndexRune(rest, p[1])
			if j < 0 {
				break
			}
			content := rest[:j]
			tags = tbAppend(tags, tbTagsText(content)...)
			if tbContains(content, "ai") {
				tags = tbAppend(tags, "ai")
			}
			rest = rest[j+len(string(p[1])):]
		}
	}
	delim, dl := -1, 0
	for _, sep := range []string{" - ", " – ", " — ", "-", "–", "—"} {
		i := strings.LastIndex(s, sep)
		if i >= 0 && i > delim {
			delim = i
			dl = len(sep)
		}
	}
	if delim >= 0 {
		tags = tbAppend(tags, tbTagsText(s[delim+dl:])...)
	}
	t := tbTrim(s)
	if t != "" && !strings.ContainsRune(")]）】", []rune(t)[len([]rune(t))-1]) {
		i := strings.LastIndexAny(t, " \t\r\n")
		token := t[i+1:]
		tags = tbAppend(tags, tbTagsText(token)...)
		if tbContains(token, "version") || tbContains(token, "ver.") {
			p := -1
			if i > 0 {
				p = strings.LastIndexAny(t[:i], " \t\r\n")
			}
			tags = tbAppend(tags, tbTagsText(t[p+1:])...)
		}
	}
	return tags
}
func tbNoise(s string) bool {
	s = tbLower(tbTrim(s))
	for _, v := range []string{"explicit", "deluxe", "digital", "premium", "album", "edit", "version", "special", "anniversary", "studio", "remastered"} {
		if s == v {
			return true
		}
	}
	return false
}
func tbFeature(s string) string {
	lower := tbLower(s)
	cut := len(s)
	for _, m := range []string{" feat.", " feat ", " ft.", " ft ", " with "} {
		i := strings.Index(lower, m)
		if i > 0 && i < cut {
			cut = i
		}
	}
	return tbTrim(s[:cut])
}
func tbSeparator(r rune) bool {
	return tbSpace(r) || r < 128 && !tbAlnum(r) || strings.ContainsRune("，。、；：！？（）【】《》「」『』—–…·", r)
}
func tbToken(s string) string {
	var out []rune
	space := true
	for _, r := range tbSimple(s) {
		if tbSeparator(r) {
			if !space {
				out = append(out, ' ')
				space = true
			}
		} else {
			out = append(out, []rune(tbLower(string(r)))[0])
			space = false
		}
	}
	return strings.TrimRight(string(out), " ")
}
func tbNormalize(s string) string {
	src := []rune(tbFeature(tbSimple(s)))
	var out []rune
	space := true
	add := func(r rune) {
		if tbSeparator(r) {
			if !space {
				out = append(out, ' ')
				space = true
			}
		} else {
			out = append(out, []rune(tbLower(string(r)))[0])
			space = false
		}
	}
	for i := 0; i < len(src); i++ {
		if close, ok := tbBrackets[src[i]]; ok {
			j := i + 1
			for j < len(src) && src[j] != close {
				j++
			}
			if j < len(src) {
				if !space {
					out = append(out, ' ')
					space = true
				}
				content := string(src[i+1 : j])
				if !tbNoise(content) {
					for _, r := range content {
						add(r)
					}
					if !space {
						out = append(out, ' ')
						space = true
					}
				}
				i = j
				continue
			}
		}
		add(src[i])
	}
	return strings.TrimRight(string(out), " ")
}
func tbStrip(s string) string {
	s = tbFeature(s)
	s = strings.TrimRight(s, " \t")
	for s != "" {
		r := []rune(s)
		var open rune
		for o, c := range tbBrackets {
			if c == r[len(r)-1] {
				open = o
				break
			}
		}
		if open == 0 {
			break
		}
		depth, pos := 0, -1
		for i := len(r) - 1; i >= 0; i-- {
			if r[i] == r[len(r)-1] {
				depth++
			} else if r[i] == open {
				depth--
				if depth == 0 {
					pos = i
					break
				}
			}
		}
		if pos <= 0 {
			break
		}
		s = strings.TrimRight(string(r[:pos]), " ")
	}
	return tbTrim(s)
}
func tbStripDelimited(s string) string {
	i, dl := -1, 0
	for _, sep := range []string{" - ", " – ", " — "} {
		p := strings.LastIndex(s, sep)
		if p > i {
			i = p
			dl = len(sep)
		}
	}
	if i < 0 || len(tbTags(s[i+dl:])) == 0 {
		return s
	}
	return tbTrim(s[:i])
}
func tbEndsArtist(t, a string) bool {
	if a == "" {
		return false
	}
	t, a = tbLower(t), tbLower(a)
	for _, s := range []string{" - ", "-", " – ", "–", " — ", "—", " / ", "/"} {
		if len(t) > len(s+a) && strings.HasSuffix(t, s+a) {
			return true
		}
	}
	return false
}
func tbDelimiter(r rune) bool { return strings.ContainsRune("/、&;；,，×", r) }
func tbSplit(s string) []string {
	var out []string
	for _, p := range strings.FieldsFunc(s, tbDelimiter) {
		if t := tbTrim(p); t != "" {
			out = append(out, t)
		}
	}
	return out
}
func tbKeyword(r []rune, i int, leading bool) int {
	if leading && (i == 0 || !tbSpace(r[i-1])) {
		return 0
	}
	for _, kw := range []string{"featuring", "feat.", "feat", "ft.", "ft", "with", "vs.", "vs"} {
		k := []rune(kw)
		if i+len(k) >= len(r) || string(r[i:i+len(k)]) != kw {
			continue
		}
		n := r[i+len(k)]
		if tbSpace(n) || strings.HasSuffix(kw, ".") && n >= 128 {
			return len(k)
		}
	}
	return 0
}
func tbNonAlias(s string) bool {
	if s == "" || tbNoise(s) {
		return true
	}
	for _, v := range strings.Split("翻唱|翻唱版|原唱|原版|合唱|合唱版|伴唱|和声|伴奏|伴奏版|纯音乐|现场|现场版|演唱会|男声|男声版|女声|女声版|童声|群星|华语群星|未知歌手|乐队|组合|声优|配音|dj版|live版|加速版|剧情版|cover|original|chorus|choir|backing vocals|vocals|vocal|instrumental|karaoke|live|concert|remix|acoustic|unplugged|demo|ver|cv|va|various|various artists|unknown|unknown artist|band|male|female|ost|cast", "|") {
		if s == v {
			return true
		}
	}
	return false
}
func tbArtists(s string, out *[][]string, depth int) {
	if depth > 4 || len(*out) > 64 {
		return
	}
	r := []rune(s)
	lower := []rune(tbLower(s))
	close := make(map[int]int)
	var stack []int
	for i, c := range r {
		if _, ok := tbBrackets[c]; ok {
			stack = append(stack, i)
		} else if len(stack) > 0 && c == tbBrackets[r[stack[len(stack)-1]]] {
			close[stack[len(stack)-1]] = i
			stack = stack[:len(stack)-1]
		}
	}
	var spans [][2]int
	start := 0
	for i := 0; i < len(r); {
		if c, ok := close[i]; ok {
			i = c + 1
			continue
		}
		n := tbKeyword(lower, i, true)
		if tbDelimiter(r[i]) {
			n = 1
		}
		if n > 0 {
			spans = append(spans, [2]int{start, i})
			i += n
			start = i
		} else {
			i++
		}
	}
	spans = append(spans, [2]int{start, len(r)})
	for _, span := range spans {
		var outer []rune
		var inners []string
		for i := span[0]; i < span[1]; {
			if c, ok := close[i]; ok {
				if len(outer) > 0 && outer[len(outer)-1] != ' ' {
					outer = append(outer, ' ')
				}
				inners = append(inners, string(r[i+1:c]))
				i = c + 1
			} else {
				outer = append(outer, r[i])
				i++
			}
		}
		var aliases, nested []string
		if token := tbToken(string(outer)); token != "" {
			aliases = append(aliases, token)
		}
		for _, in := range inners {
			in = tbTrim(in)
			if in == "" {
				continue
			}
			kw := tbKeyword([]rune(tbLower(in)), 0, false)
			if kw > 0 {
				rest := tbTrim(string([]rune(in)[kw:]))
				if rest != "" {
					nested = append(nested, rest)
				}
				continue
			}
			for _, p := range tbSplit(in) {
				norm := tbToken(p)
				if !tbNonAlias(norm) {
					aliases = tbAppend(aliases, norm)
				}
			}
		}
		if len(aliases) > 0 {
			*out = append(*out, aliases)
		}
		for _, n := range nested {
			tbArtists(n, out, depth+1)
		}
		if len(*out) > 64 {
			return
		}
	}
}
func tbAlias(a, b []string) int {
	shared := 0
	for _, x := range a {
		for _, y := range b {
			if x == y {
				shared++
				break
			}
		}
	}
	if shared == 0 {
		return 0
	}
	if shared == len(a) && shared == len(b) {
		return 100
	}
	return 95
}
func tbAssignment(a, b [][]string) int {
	n := max(len(a), len(b))
	score := make([][]int, n+1)
	for i := range score {
		score[i] = make([]int, n+1)
	}
	for i, x := range a {
		for j, y := range b {
			score[i+1][j+1] = tbAlias(x, y)
		}
	}
	u, v, p, way := make([]int, n+1), make([]int, n+1), make([]int, n+1), make([]int, n+1)
	for row := 1; row <= n; row++ {
		p[0] = row
		col := 0
		cost := make([]int, n+1)
		visited := make([]bool, n+1)
		for i := range cost {
			cost[i] = math.MaxInt32
		}
		for {
			visited[col] = true
			r := p[col]
			delta, next := math.MaxInt32, 0
			for c := 1; c <= n; c++ {
				if visited[c] {
					continue
				}
				cur := 100 - score[r][c] - u[r] - v[c]
				if cur < cost[c] {
					cost[c] = cur
					way[c] = col
				}
				if cost[c] < delta {
					delta = cost[c]
					next = c
				}
			}
			for c := 0; c <= n; c++ {
				if visited[c] {
					u[p[c]] += delta
					v[c] -= delta
				} else {
					cost[c] -= delta
				}
			}
			col = next
			if p[col] == 0 {
				break
			}
		}
		for col != 0 {
			prev := way[col]
			p[col] = p[prev]
			col = prev
		}
	}
	sum := 0
	for c := 1; c <= n; c++ {
		sum += score[p[c]][c]
	}
	return sum
}
func tbJaro(a, b string) float64 {
	x, y := []rune(a), []rune(b)
	if len(x) == 0 || len(y) == 0 {
		return 0
	}
	if a == b {
		return 1
	}
	dist := max(0, max(len(x), len(y))/2-1)
	am, bm := make([]bool, len(x)), make([]bool, len(y))
	matches := 0
	for i, c := range x {
		for j := max(0, i-dist); j < min(len(y), i+dist+1); j++ {
			if !bm[j] && c == y[j] {
				am[i] = true
				bm[j] = true
				matches++
				break
			}
		}
	}
	if matches == 0 {
		return 0
	}
	trans, j := 0, 0
	for i, c := range x {
		if !am[i] {
			continue
		}
		for !bm[j] {
			j++
		}
		if c != y[j] {
			trans++
		}
		j++
	}
	m := float64(matches)
	s := (m/float64(len(x)) + m/float64(len(y)) + (m-float64(trans)/2)/m) / 3
	if s > 0.7 {
		p := 0
		for p < min(len(x), len(y), 4) && x[p] == y[p] {
			p++
		}
		s += float64(p) * 0.1 * (1 - s)
	}
	return min(1, max(0, s))
}
func tbSinger(a, b string) int {
	if a == "" || b == "" || len([]rune(a)) > 1024 || len([]rune(b)) > 1024 {
		return 0
	}
	if a == b {
		return 100
	}
	var pa, pb [][]string
	tbArtists(a, &pa, 0)
	tbArtists(b, &pb, 0)
	if len(pa) > 64 || len(pb) > 64 {
		return 0
	}
	count := max(len(pa), len(pb))
	if len(pa) > 1 && len(pb) > 1 {
		return tbAssignment(pa, pb) / count
	}
	na, nb := tbNormalize(a), tbNormalize(b)
	if na == nb {
		return 100
	}
	for _, p := range []*string{&na, &nb} {
		*p = " " + *p + " "
		for _, kw := range []string{" feat. ", " feat ", " ft. ", " ft ", " featuring ", " with ", " vs ", " vs. "} {
			*p = strings.ReplaceAll(*p, kw, " / ")
		}
		*p = tbTrim(*p)
	}
	if na == nb {
		return 100
	}
	if strings.Contains(na, nb) || strings.Contains(nb, na) {
		return 90
	}
	if len(pa) == 0 || len(pb) == 0 {
		return 0
	}
	sum := tbAssignment(pa, pb)
	if sum == 0 {
		return 0
	}
	return max(sum/count, int(math.Round(tbJaro(na, nb)*100)))
}
func tbScore(r request, h hit) int {
	title, artist, name, singer := tbWide(r.Title), tbWide(r.Artist), tbWide(h.Title), tbWide(strings.Join(h.Artists, "/"))
	unknown := func(s string) bool { s = tbLower(tbTrim(s)); return s == "" || s == "unknown title" }
	if unknown(title) || unknown(name) || len([]rune(artist)) > 1024 || len([]rune(singer)) > 1024 {
		return 0
	}
	a, b := tbNormalize(title), tbNormalize(name)
	if a == "" || b == "" {
		return 0
	}
	sim := tbJaro(a, b)
	if sim < 0.5 {
		return 0
	}
	t := float64(math.Round(sim*100)) / 100
	useful := func(s string) bool {
		s = tbLower(tbTrim(s))
		return s != "" && s != "unknown artist" && s != "未知歌手"
	}
	hasArtist := useful(artist) && useful(singer)
	s := 0.
	if hasArtist {
		s = float64(tbSinger(artist, singer)) / 100
		if s == 0 {
			return 0
		}
	}
	diff := math.Abs(float64(r.DurationMS - h.DurationMS))
	hasDuration := r.DurationMS > 0 && h.DurationMS > 0
	if hasDuration && diff > 15000 {
		return 0
	}
	qa, ca := tbTags(title), tbTags(name)
	sort.Strings(qa)
	sort.Strings(ca)
	if strings.Join(qa, "|") != strings.Join(ca, "|") {
		return 0
	}
	d := 0.
	if hasDuration {
		if diff <= 1000 {
			d = 1
		} else if diff < 10000 {
			d = 1 - (diff-1000)/9000
		}
	}
	total := t
	if hasArtist && hasDuration {
		total = t*.5 + s*.3 + d*.2
	} else if hasArtist {
		total = t*.6 + s*.4
	} else if hasDuration {
		total = t*.75 + d*.25
	}
	return int(math.Round(min(1, max(0, total)) * 100))
}
func tbPrimary(a string) string {
	n := tbNormalize(a)
	for _, kw := range []string{" feat ", " ft ", " featuring ", " with ", " vs "} {
		n = strings.ReplaceAll(n, kw, "/")
	}
	if p := tbSplit(n); len(p) > 1 {
		return p[0]
	}
	if p := tbSplit(a); len(p) > 0 {
		return tbTrim(p[0])
	}
	return tbTrim(a)
}
func tbVariants(r request) []hit {
	t, a := tbWide(r.Title), tbWide(r.Artist)
	var out []hit
	add := func(t, a string) {
		t, a = tbTrim(t), tbTrim(a)
		if t == "" {
			return
		}
		t, a = tbUTF8(t), tbUTF8(a)
		for _, h := range out {
			if h.Title == t && strings.Join(h.Artists, "/") == a {
				return
			}
		}
		q := t
		if a != "" && !tbEndsArtist(tbWide(t), tbWide(a)) {
			q += " " + a
		}
		out = append(out, hit{Title: t, Artists: []string{a}, Extra: map[string]string{"query": q}})
	}
	add(t, a)
	add(tbSimple(t), tbSimple(a))
	add(tbNormalize(t), tbNormalize(a))
	add(t, tbPrimary(a))
	add(tbStrip(t), a)
	add(tbStripDelimited(t), a)
	count := len(out)
	for i := 0; i < count; i++ {
		h := out[i]
		add(tbSimple(tbWide(h.Title)), tbSimple(tbWide(strings.Join(h.Artists, "/"))))
	}
	return out
}
