// Package mailbox implements direct, read-only IMAP over TLS. There is no
// helper server, notification database access, or persistence of mail content.
package mailbox

import (
	"bytes"
	"context"
	"crypto/tls"
	"errors"
	"fmt"
	"io"
	"mime"
	"net"
	"net/mail"
	"sort"
	"strings"
	"time"
	"unicode"

	imap "github.com/emersion/go-imap/v2"
	"github.com/emersion/go-imap/v2/imapclient"
	"github.com/emersion/go-message/charset"
	message "github.com/emersion/go-message/mail"
	"golang.org/x/net/html"
)

const bodyLimit = 48 * 1024

type Account struct{ Address, Server, Secret string }
type Message struct {
	UID           uint32
	Subject, From string
	Date          time.Time
}
type Inbox struct {
	Validity uint32
	Messages []Message
}

func Validate(account Account) (Account, error) {
	account.Address = strings.TrimSpace(account.Address)
	account.Server = strings.ToLower(strings.TrimSpace(account.Server))
	addr, err := mail.ParseAddress(account.Address)
	if err != nil || addr.Address != account.Address || len(account.Address) > 254 || len(account.Secret) < 1 || len(account.Secret) > 1024 || strings.ContainsAny(account.Secret, "\r\n\x00") {
		return Account{}, errors.New("请填写完整的邮箱地址和 IMAP 授权码")
	}
	_, domain, _ := strings.Cut(account.Address, "@")
	if account.Server == "" {
		switch strings.ToLower(domain) {
		case "qq.com", "foxmail.com":
			account.Server = "imap.qq.com"
		case "163.com":
			account.Server = "imap.163.com"
		case "126.com":
			account.Server = "imap.126.com"
		case "gmail.com":
			account.Server = "imap.gmail.com"
		default:
			return Account{}, errors.New("请填写服务商提供的 IMAP 服务器域名")
		}
	}
	if len(account.Server) > 253 || !strings.Contains(account.Server, ".") || net.ParseIP(account.Server) != nil || strings.ContainsFunc(account.Server, func(r rune) bool { return !(r >= 'a' && r <= 'z' || r >= '0' && r <= '9' || r == '-' || r == '.') }) {
		return Account{}, errors.New("IMAP 服务器应为域名，不带端口或路径")
	}
	return account, nil
}

func connect(ctx context.Context, account Account) (*imapclient.Client, func(), error) {
	dialer := tls.Dialer{NetDialer: &net.Dialer{Timeout: 8 * time.Second}, Config: &tls.Config{MinVersion: tls.VersionTLS12, ServerName: account.Server}}
	conn, err := dialer.DialContext(ctx, "tcp", net.JoinHostPort(account.Server, "993"))
	if err != nil {
		return nil, nil, errors.New("未连接到邮箱服务器，请核对 IMAP 服务器和网络")
	}
	client := imapclient.New(conn, &imapclient.Options{WordDecoder: &mime.WordDecoder{CharsetReader: charset.Reader}})
	stop := context.AfterFunc(ctx, func() { _ = conn.Close() })
	close := func() { stop(); _ = client.Close() }
	if err := client.Login(account.Address, account.Secret).Wait(); err != nil {
		close()
		return nil, nil, errors.New("邮箱登录失败，请检查 IMAP 是否启用以及授权码是否正确")
	}
	return client, close, nil
}

func Recent(ctx context.Context, account Account) (Inbox, error) {
	c, close, err := connect(ctx, account)
	if err != nil {
		return Inbox{}, err
	}
	defer close()
	return readRecent(c)
}

func readRecent(c *imapclient.Client) (Inbox, error) {
	data, err := c.Select("INBOX", &imap.SelectOptions{ReadOnly: true}).Wait()
	if err != nil {
		return Inbox{}, errors.New("无法读取收件箱")
	}
	out := Inbox{Validity: data.UIDValidity}
	if data.NumMessages == 0 {
		return out, nil
	}
	start := uint32(1)
	if data.NumMessages > 12 {
		start = data.NumMessages - 11
	}
	var ids imap.SeqSet
	ids.AddRange(start, data.NumMessages)
	rows, err := c.Fetch(ids, &imap.FetchOptions{UID: true, Envelope: true, InternalDate: true}).Collect()
	if err != nil {
		return Inbox{}, errors.New("无法读取最近邮件")
	}
	for _, row := range rows {
		if row.UID == 0 || row.Envelope == nil {
			continue
		}
		name := "未知发件人"
		if from := row.Envelope.From; len(from) > 0 {
			name = from[0].Addr()
			if from[0].Name != "" {
				name = from[0].Name + " <" + name + ">"
			}
		}
		out.Messages = append(out.Messages, Message{UID: uint32(row.UID), Subject: clean(row.Envelope.Subject, 180), From: clean(name, 180), Date: row.InternalDate})
	}
	sort.Slice(out.Messages, func(i, j int) bool { return out.Messages[i].UID > out.Messages[j].UID })
	return out, nil
}

func Body(ctx context.Context, account Account, validity uint32, uid uint32) (string, error) {
	if validity == 0 || uid == 0 {
		return "", errors.New("请刷新收件箱后重试")
	}
	c, close, err := connect(ctx, account)
	if err != nil {
		return "", err
	}
	defer close()
	return readBody(c, validity, uid)
}

func readBody(c *imapclient.Client, validity, uid uint32) (string, error) {
	data, err := c.Select("INBOX", &imap.SelectOptions{ReadOnly: true}).Wait()
	if err != nil || data.UIDValidity != validity {
		return "", errors.New("收件箱已变化，请刷新")
	}
	section := &imap.FetchItemBodySection{Peek: true, Partial: &imap.SectionPartial{Size: bodyLimit}}
	cmd := c.Fetch(imap.UIDSetNum(imap.UID(uid)), &imap.FetchOptions{UID: true, BodySection: []*imap.FetchItemBodySection{section}})
	defer cmd.Close()
	row := cmd.Next()
	if row == nil {
		return "", errors.New("邮件已删除或无法读取")
	}
	for {
		item := row.Next()
		if item == nil {
			break
		}
		if body, ok := item.(imapclient.FetchItemDataBodySection); ok {
			raw, err := io.ReadAll(io.LimitReader(body.Literal, bodyLimit))
			if err != nil {
				return "", errors.New("无法读取邮件正文")
			}
			return preview(raw), nil
		}
	}
	return "", errors.New("邮件没有可预览的正文")
}

func clean(s string, max int) string {
	s = strings.Map(func(r rune) rune {
		if unicode.IsSpace(r) {
			return ' '
		}
		if unicode.IsControl(r) || unicode.Is(unicode.Cf, r) {
			return -1
		}
		return r
	}, s)
	s = strings.Join(strings.Fields(s), " ")
	runes := []rune(s)
	if len(runes) > max {
		s = string(runes[:max]) + "…"
	}
	return s
}

func preview(raw []byte) string {
	r, err := message.CreateReader(bytes.NewReader(raw))
	if r == nil {
		return ""
	}
	defer r.Close()
	_ = err // Unsupported charset can still give readable UTF-8 on another part.
	var htmlFallback string
	for count := 0; count < 20; count++ {
		part, err := r.NextPart()
		if err != nil || part == nil {
			break
		}
		h, ok := part.Header.(*message.InlineHeader)
		if !ok {
			continue
		}
		media, _, _ := h.ContentType()
		if media == "text/plain" || media == "text/html" || media == "" {
			raw, _ := io.ReadAll(io.LimitReader(part.Body, bodyLimit))
			if media == "text/html" {
				if htmlFallback == "" {
					htmlFallback = plainHTML(raw)
				}
			} else if s := clean(strings.ToValidUTF8(string(raw), "�"), 1600); s != "" {
				return s
			}
		}
	}
	return clean(htmlFallback, 1600)
}

func plainHTML(raw []byte) string {
	tokens := html.NewTokenizer(bytes.NewReader(raw))
	var out strings.Builder
	var hidden int
	for {
		switch tokens.Next() {
		case html.ErrorToken:
			return out.String()
		case html.TextToken:
			if hidden == 0 {
				fmt.Fprintf(&out, "%s ", tokens.Text())
			}
		case html.StartTagToken:
			tag, _ := tokens.TagName()
			if string(tag) == "script" || string(tag) == "style" || string(tag) == "head" {
				hidden++
			}
		case html.EndTagToken:
			tag, _ := tokens.TagName()
			if hidden > 0 && (string(tag) == "script" || string(tag) == "style" || string(tag) == "head") {
				hidden--
			}
		}
	}
}
