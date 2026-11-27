// Port of taskbar-lyrics-displayer lyricsfile_parser.cpp (Apache-2.0).
package lyrics

import (
	yaml "go.yaml.in/yaml/v3"
	"io"
	"sort"
	"strconv"
	"strings"
	"unicode/utf8"
)

type tbWord struct {
	At    int64  `json:"at"`
	Until int64  `json:"until"`
	Text  string `json:"text"`
}
type tbLine struct {
	At    int64    `json:"at"`
	Until int64    `json:"until"`
	Text  string   `json:"text"`
	Words []tbWord `json:"words"`
}
type tbDocument struct {
	Lines        []tbLine `json:"lines"`
	Timed        bool     `json:"timed"`
	Instrumental bool     `json:"instrumental"`
}

func tbMapping(n *yaml.Node) map[string]*yaml.Node {
	if n == nil || n.Kind != yaml.MappingNode {
		return nil
	}
	m := map[string]*yaml.Node{}
	for i := 0; i < len(n.Content); i += 2 {
		k := n.Content[i]
		if k.Kind != yaml.ScalarNode || m[k.Value] != nil {
			return nil
		}
		m[k.Value] = n.Content[i+1]
	}
	return m
}
func tbScalar(n *yaml.Node) string {
	if n == nil || n.Kind != yaml.ScalarNode || n.Tag == "!!null" {
		return ""
	}
	return n.Value
}
func tbInt(n *yaml.Node) int64 {
	v, err := strconv.ParseInt(tbScalar(n), 10, 64)
	if err != nil {
		return -1
	}
	return v
}
func tbText(n *yaml.Node, budget *int) string {
	s := tbScalar(n)
	if len(s) == 0 || len(s) > 4096 || len(s) > *budget {
		return ""
	}
	*budget -= len(s)
	if !utf8.ValidString(s) {
		return ""
	}
	return s
}
func tbReadWords(n *yaml.Node, end int64, budget *int) []tbWord {
	if n == nil || n.Kind != yaml.SequenceNode || len(n.Content) > 1024 {
		return nil
	}
	var out []tbWord
	for _, word := range n.Content {
		m := tbMapping(word)
		if m == nil {
			return nil
		}
		start := tbInt(m["start_ms"])
		endNode := m["end_ms"]
		hasEnd := endNode != nil && endNode.Tag != "!!null"
		until := int64(-1)
		if hasEnd {
			until = tbInt(endNode)
		}
		text := tbText(m["text"], budget)
		if start < 0 || hasEnd && until < start || text == "" {
			return nil
		}
		out = append(out, tbWord{start, until, text})
	}
	for i := range out {
		if i > 0 && out[i].At < out[i-1].At {
			return nil
		}
		if out[i].Until < 0 {
			next := end
			if i+1 < len(out) {
				next = out[i+1].At
			}
			out[i].Until = max(out[i].At, next)
		}
	}
	return out
}
func tbLyricsfile(s string) *tbDocument {
	if s == "" || len(s) > 512*1024 {
		return nil
	}
	d := yaml.NewDecoder(strings.NewReader(s))
	var doc, next yaml.Node
	if d.Decode(&doc) != nil || d.Decode(&next) != io.EOF || len(doc.Content) != 1 {
		return nil
	}
	root := tbMapping(doc.Content[0])
	if root == nil || tbScalar(root["version"]) != "1.0" {
		return nil
	}
	lines := root["lines"]
	if lines == nil || lines.Kind != yaml.SequenceNode || len(lines.Content) > 4096 {
		return nil
	}
	result := &tbDocument{Timed: true}
	budget := 512 * 1024
	for _, node := range lines.Content {
		m := tbMapping(node)
		if m == nil {
			continue
		}
		start := tbInt(m["start_ms"])
		if start < 0 {
			continue
		}
		line := tbLine{At: start}
		line.Words = tbReadWords(m["words"], tbInt(m["end_ms"]), &budget)
		if line.Words == nil {
			line.Words = []tbWord{}
		}
		if len(line.Words) == 0 {
			line.Text = tbText(m["text"], &budget)
		} else {
			for _, w := range line.Words {
				line.Text += w.Text
			}
			line.Until = line.Words[len(line.Words)-1].Until
		}
		if line.Text != "" {
			result.Lines = append(result.Lines, line)
		}
	}
	sort.SliceStable(result.Lines, func(i, j int) bool { return result.Lines[i].At < result.Lines[j].At })
	if len(result.Lines) == 0 {
		return nil
	}
	return result
}
