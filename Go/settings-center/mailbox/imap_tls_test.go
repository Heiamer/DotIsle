package mailbox

import (
	"crypto/ed25519"
	"crypto/rand"
	"crypto/tls"
	"crypto/x509"
	"fmt"
	"math/big"
	"net"
	"testing"
	"time"

	imap "github.com/emersion/go-imap/v2"
	"github.com/emersion/go-imap/v2/imapclient"
	"github.com/emersion/go-imap/v2/imapserver"
	"github.com/emersion/go-imap/v2/imapserver/imapmemserver"
)

func TestReadOnlyInboxAndNewMailOnLocalTLS(t *testing.T) {
	now := time.Now()
	public, private, err := ed25519.GenerateKey(rand.Reader)
	if err != nil {
		t.Fatal(err)
	}
	template := &x509.Certificate{SerialNumber: big.NewInt(1), NotBefore: now.Add(-time.Minute), NotAfter: now.Add(time.Hour), DNSNames: []string{"mail.test"}, KeyUsage: x509.KeyUsageDigitalSignature, ExtKeyUsage: []x509.ExtKeyUsage{x509.ExtKeyUsageServerAuth}}
	der, err := x509.CreateCertificate(rand.Reader, template, template, public, private)
	if err != nil {
		t.Fatal(err)
	}
	cert, err := x509.ParseCertificate(der)
	if err != nil {
		t.Fatal(err)
	}
	pool := x509.NewCertPool()
	pool.AddCert(cert)
	listener, err := tls.Listen("tcp", "127.0.0.1:0", &tls.Config{MinVersion: tls.VersionTLS12, Certificates: []tls.Certificate{{Certificate: [][]byte{der}, PrivateKey: private}}})
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = listener.Close() })
	memory := imapmemserver.New()
	user := imapmemserver.NewUser("inbox@mail.test", "fixture-secret")
	if err := user.Create("INBOX", nil); err != nil {
		t.Fatal(err)
	}
	memory.AddUser(user)
	server := imapserver.New(&imapserver.Options{NewSession: func(*imapserver.Conn) (imapserver.Session, *imapserver.GreetingData, error) {
		return memory.NewSession(), nil, nil
	}})
	t.Cleanup(func() { _ = server.Close() })
	go server.Serve(listener)
	connect := func() *imapclient.Client {
		conn, err := tls.DialWithDialer(&net.Dialer{Timeout: 3 * time.Second}, "tcp", listener.Addr().String(), &tls.Config{MinVersion: tls.VersionTLS12, ServerName: "mail.test", RootCAs: pool})
		if err != nil {
			t.Fatal(err)
		}
		_ = conn.SetDeadline(time.Now().Add(8 * time.Second))
		c := imapclient.New(conn, nil)
		t.Cleanup(func() { _ = c.Close() })
		if err := c.Login("inbox@mail.test", "fixture-secret").Wait(); err != nil {
			t.Fatal(err)
		}
		return c
	}
	writer := connect()
	appendMail := func(subject, body string) {
		raw := fmt.Sprintf("From: Fixture <sender@mail.test>\r\nSubject: %s\r\nMIME-Version: 1.0\r\nContent-Type: text/plain; charset=utf-8\r\n\r\n%s\r\n", subject, body)
		cmd := writer.Append("INBOX", int64(len(raw)), nil)
		if _, err := cmd.Write([]byte(raw)); err != nil {
			t.Fatal(err)
		}
		if err := cmd.Close(); err != nil {
			t.Fatal(err)
		}
		if _, err := cmd.Wait(); err != nil {
			t.Fatal(err)
		}
	}
	for i := 0; i < 13; i++ {
		appendMail("Archive", "Old mail")
	}
	first, err := readRecent(connect())
	if err != nil || first.Validity == 0 || len(first.Messages) != 12 {
		t.Fatalf("baseline: %+v %v", first, err)
	}
	appendMail("New verification message", "Your verification code is 452316.")
	second, err := readRecent(connect())
	if err != nil || len(second.Messages) != 12 || second.Validity != first.Validity {
		t.Fatalf("after append: %+v %v", second, err)
	}
	msg := second.Messages[0]
	if msg.UID <= first.Messages[0].UID || msg.Subject != "New verification message" || msg.From != "Fixture <sender@mail.test>" {
		t.Fatalf("newest: %+v", msg)
	}
	text, err := readBody(connect(), second.Validity, msg.UID)
	if err != nil || text != "Your verification code is 452316." {
		t.Fatalf("preview: %q %v", text, err)
	}
	if _, err := readBody(connect(), second.Validity+1, msg.UID); err == nil {
		t.Fatal("accepted a changed UIDVALIDITY")
	}
	inspector := connect()
	if _, err := inspector.Select("INBOX", nil).Wait(); err != nil {
		t.Fatal(err)
	}
	rows, err := inspector.Fetch(imap.UIDSetNum(imap.UID(msg.UID)), &imap.FetchOptions{Flags: true}).Collect()
	if err != nil || len(rows) != 1 {
		t.Fatalf("flags: %+v %v", rows, err)
	}
	for _, flag := range rows[0].Flags {
		if flag == imap.FlagSeen {
			t.Fatal("read-only preview marked message Seen")
		}
	}
}
