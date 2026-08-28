package rivun

import (
	"crypto/rand"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"net"
	"strings"
	"time"
	"unicode/utf8"
)

const (
	EnvelopeMagic                = "ZENV"
	EnvelopeVersion              = uint16(1)
	EnvelopeHeaderLen            = 74
	MaxSubjectLen                = 512
	MaxContentTypeLen            = 128
	MaxMetadataLen               = 64 * 1024
	MaxBodyLen                   = 16 * 1024 * 1024
	DefaultContentType           = "application/octet-stream"
	RegistryIndexContentType     = "application/rivun-registry-index+json"
	RegistryBundleContentType    = "application/rivun-registry-bundle-manifest+json"
	RegistryIndexRequestSubject  = "rivun.registry.index.request"
	RegistryIndexResponseSubject = "rivun.registry.index.response"
	BundleManifestRequestSubject = "rivun.registry.bundle.manifest.request"
	BundleManifestResponseSubject = "rivun.registry.bundle.manifest.response"
)

type MessageKind uint16

const (
	KindData MessageKind = iota + 1
	KindEvent
	KindCommand
	KindQuery
	KindResponse
	KindStreamChunk
	KindAction
	KindControl
)

type UUID [16]byte

func NewUUIDV4() (UUID, error) {
	var id UUID
	if _, err := rand.Read(id[:]); err != nil {
		return UUID{}, err
	}
	id[6] = (id[6] & 0x0f) | 0x40
	id[8] = (id[8] & 0x3f) | 0x80
	return id, nil
}

func ParseUUID(value string) (UUID, error) {
	var id UUID
	compact := strings.ReplaceAll(value, "-", "")
	if len(compact) != 32 {
		return UUID{}, fmt.Errorf("invalid UUID %q", value)
	}
	raw, err := hex.DecodeString(compact)
	if err != nil {
		return UUID{}, err
	}
	copy(id[:], raw)
	return id, nil
}

func (id UUID) String() string {
	hexed := hex.EncodeToString(id[:])
	return fmt.Sprintf("%s-%s-%s-%s-%s", hexed[0:8], hexed[8:12], hexed[12:16], hexed[16:20], hexed[20:32])
}

func (id UUID) MarshalJSON() ([]byte, error) {
	return json.Marshal(id.String())
}

func (id *UUID) UnmarshalJSON(input []byte) error {
	var value string
	if err := json.Unmarshal(input, &value); err != nil {
		return err
	}
	parsed, err := ParseUUID(value)
	if err != nil {
		return err
	}
	*id = parsed
	return nil
}

func (kind MessageKind) RequiresSubject() bool {
	return kind != KindData
}

type Envelope struct {
	Kind          MessageKind
	ID            UUID
	CorrelationID *UUID
	CausationID   *UUID
	Subject       string
	ContentType   string
	Metadata      []byte
	Body          []byte
}

func NewEnvelope(kind MessageKind, subject string, contentType string, body []byte) (Envelope, error) {
	id, err := NewUUIDV4()
	if err != nil {
		return Envelope{}, err
	}
	env := Envelope{
		Kind:        kind,
		ID:          id,
		Subject:     subject,
		ContentType: contentType,
		Body:        append([]byte(nil), body...),
	}
	return env, env.Validate()
}

func (env Envelope) Validate() error {
	if !utf8.ValidString(env.Subject) {
		return errors.New("subject must be valid UTF-8")
	}
	if !utf8.ValidString(env.ContentType) {
		return errors.New("content_type must be valid UTF-8")
	}
	return validateLengths(env.Kind, len([]byte(env.Subject)), len([]byte(env.ContentType)), len(env.Metadata), len(env.Body))
}

func (env Envelope) Encode() ([]byte, error) {
	if err := env.Validate(); err != nil {
		return nil, err
	}
	subject := []byte(env.Subject)
	contentType := []byte(env.ContentType)
	total := EnvelopeHeaderLen + len(subject) + len(contentType) + len(env.Metadata) + len(env.Body)
	out := make([]byte, total)
	copy(out[0:4], []byte(EnvelopeMagic))
	putU16(out[4:6], EnvelopeVersion)
	putU16(out[6:8], uint16(env.Kind))
	putU16(out[8:10], 0)
	copy(out[10:26], env.ID[:])
	if env.CorrelationID != nil {
		copy(out[26:42], env.CorrelationID[:])
	}
	if env.CausationID != nil {
		copy(out[42:58], env.CausationID[:])
	}
	putU16(out[58:60], uint16(len(subject)))
	putU16(out[60:62], uint16(len(contentType)))
	putU32(out[62:66], uint32(len(env.Metadata)))
	putU64(out[66:74], uint64(len(env.Body)))
	offset := EnvelopeHeaderLen
	copy(out[offset:], subject)
	offset += len(subject)
	copy(out[offset:], contentType)
	offset += len(contentType)
	copy(out[offset:], env.Metadata)
	offset += len(env.Metadata)
	copy(out[offset:], env.Body)
	return out, nil
}

func DecodeEnvelope(input []byte) (Envelope, error) {
	if len(input) < EnvelopeHeaderLen {
		return Envelope{}, fmt.Errorf("envelope too short: expected at least %d, got %d", EnvelopeHeaderLen, len(input))
	}
	if string(input[0:4]) != EnvelopeMagic {
		return Envelope{}, errors.New("invalid envelope magic")
	}
	if version := readU16(input[4:6]); version != EnvelopeVersion {
		return Envelope{}, fmt.Errorf("unsupported envelope version %d", version)
	}
	kind := MessageKind(readU16(input[6:8]))
	if kind < KindData || kind > KindControl {
		return Envelope{}, fmt.Errorf("unknown envelope kind %d", kind)
	}
	if reserved := readU16(input[8:10]); reserved != 0 {
		return Envelope{}, fmt.Errorf("reserved envelope field must be zero, got %d", reserved)
	}
	var id UUID
	copy(id[:], input[10:26])
	correlationID := optionalUUID(input[26:42])
	causationID := optionalUUID(input[42:58])
	subjectLen := int(readU16(input[58:60]))
	contentTypeLen := int(readU16(input[60:62]))
	metadataLen := int(readU32(input[62:66]))
	bodyLenRaw := readU64(input[66:74])
	if bodyLenRaw > uint64(MaxBodyLen) {
		return Envelope{}, fmt.Errorf("body length exceeds maximum %d", MaxBodyLen)
	}
	bodyLen := int(bodyLenRaw)
	if err := validateLengths(kind, subjectLen, contentTypeLen, metadataLen, bodyLen); err != nil {
		return Envelope{}, err
	}
	expected := EnvelopeHeaderLen + subjectLen + contentTypeLen + metadataLen + bodyLen
	if len(input) != expected {
		return Envelope{}, fmt.Errorf("envelope length mismatch: expected %d, got %d", expected, len(input))
	}
	subjectStart := EnvelopeHeaderLen
	contentTypeStart := subjectStart + subjectLen
	metadataStart := contentTypeStart + contentTypeLen
	bodyStart := metadataStart + metadataLen
	if !utf8.Valid(input[subjectStart:contentTypeStart]) {
		return Envelope{}, errors.New("invalid UTF-8 in subject")
	}
	if !utf8.Valid(input[contentTypeStart:metadataStart]) {
		return Envelope{}, errors.New("invalid UTF-8 in content_type")
	}
	return Envelope{
		Kind:          kind,
		ID:            id,
		CorrelationID: correlationID,
		CausationID:   causationID,
		Subject:       string(input[subjectStart:contentTypeStart]),
		ContentType:   string(input[contentTypeStart:metadataStart]),
		Metadata:      append([]byte(nil), input[metadataStart:bodyStart]...),
		Body:          append([]byte(nil), input[bodyStart:]...),
	}, nil
}

type ControlFrame struct {
	Subject       string
	ContentType   string
	Body          []byte
	Metadata      []byte
	ID            UUID
	CorrelationID *UUID
	CausationID   *UUID
}

func NewControlFrame(subject string, contentType string, body []byte) (ControlFrame, error) {
	id, err := NewUUIDV4()
	if err != nil {
		return ControlFrame{}, err
	}
	frame := ControlFrame{
		Subject:     subject,
		ContentType: contentType,
		Body:        append([]byte(nil), body...),
		ID:          id,
	}
	_, err = frame.ToEnvelope()
	return frame, err
}

func NewJSONControlFrame(subject string, contentType string, payload any) (ControlFrame, error) {
	body, err := json.Marshal(payload)
	if err != nil {
		return ControlFrame{}, err
	}
	return NewControlFrame(subject, contentType, body)
}

func (frame ControlFrame) ToEnvelope() (Envelope, error) {
	env := Envelope{
		Kind:          KindControl,
		ID:            frame.ID,
		CorrelationID: frame.CorrelationID,
		CausationID:   frame.CausationID,
		Subject:       frame.Subject,
		ContentType:   frame.ContentType,
		Metadata:      append([]byte(nil), frame.Metadata...),
		Body:          append([]byte(nil), frame.Body...),
	}
	return env, env.Validate()
}

func (frame ControlFrame) Encode() ([]byte, error) {
	env, err := frame.ToEnvelope()
	if err != nil {
		return nil, err
	}
	return env.Encode()
}

func (frame ControlFrame) JSONBody(out any) error {
	return json.Unmarshal(frame.Body, out)
}

func DecodeControlFrame(input []byte) (ControlFrame, error) {
	env, err := DecodeEnvelope(input)
	if err != nil {
		return ControlFrame{}, err
	}
	if env.Kind != KindControl {
		return ControlFrame{}, fmt.Errorf("expected control envelope, got kind %d", env.Kind)
	}
	return ControlFrame{
		Subject:       env.Subject,
		ContentType:   env.ContentType,
		Body:          env.Body,
		Metadata:      env.Metadata,
		ID:            env.ID,
		CorrelationID: env.CorrelationID,
		CausationID:   env.CausationID,
	}, nil
}

type UDPClient struct {
	conn *net.UDPConn
}

func NewUDPClient(localAddr string) (*UDPClient, error) {
	addr, err := net.ResolveUDPAddr("udp", localAddr)
	if err != nil {
		return nil, err
	}
	conn, err := net.ListenUDP("udp", addr)
	if err != nil {
		return nil, err
	}
	return &UDPClient{conn: conn}, nil
}

func (client *UDPClient) LocalAddr() net.Addr {
	return client.conn.LocalAddr()
}

func (client *UDPClient) SendEnvelope(env Envelope, target string) (int, error) {
	addr, err := net.ResolveUDPAddr("udp", target)
	if err != nil {
		return 0, err
	}
	payload, err := env.Encode()
	if err != nil {
		return 0, err
	}
	return client.conn.WriteToUDP(payload, addr)
}

func (client *UDPClient) SendControl(frame ControlFrame, target string) (int, error) {
	env, err := frame.ToEnvelope()
	if err != nil {
		return 0, err
	}
	return client.SendEnvelope(env, target)
}

func (client *UDPClient) RecvEnvelope(maxBytes int, timeout time.Duration) (Envelope, *net.UDPAddr, error) {
	if maxBytes <= 0 {
		maxBytes = 65535
	}
	if timeout > 0 {
		if err := client.conn.SetReadDeadline(time.Now().Add(timeout)); err != nil {
			return Envelope{}, nil, err
		}
	}
	buf := make([]byte, maxBytes)
	n, addr, err := client.conn.ReadFromUDP(buf)
	if err != nil {
		return Envelope{}, nil, err
	}
	env, err := DecodeEnvelope(buf[:n])
	return env, addr, err
}

func (client *UDPClient) RequestControl(frame ControlFrame, target string, timeout time.Duration) (ControlFrame, error) {
	if _, err := client.SendControl(frame, target); err != nil {
		return ControlFrame{}, err
	}
	env, _, err := client.RecvEnvelope(65535, timeout)
	if err != nil {
		return ControlFrame{}, err
	}
	if env.Kind != KindControl {
		return ControlFrame{}, fmt.Errorf("expected control response, got kind %d", env.Kind)
	}
	encoded, err := env.Encode()
	if err != nil {
		return ControlFrame{}, err
	}
	return DecodeControlFrame(encoded)
}

func (client *UDPClient) Close() error {
	return client.conn.Close()
}

func validateLengths(kind MessageKind, subjectLen int, contentTypeLen int, metadataLen int, bodyLen int) error {
	if subjectLen > MaxSubjectLen {
		return fmt.Errorf("subject length exceeds maximum %d", MaxSubjectLen)
	}
	if contentTypeLen > MaxContentTypeLen {
		return fmt.Errorf("content_type length exceeds maximum %d", MaxContentTypeLen)
	}
	if metadataLen > MaxMetadataLen {
		return fmt.Errorf("metadata length exceeds maximum %d", MaxMetadataLen)
	}
	if bodyLen > MaxBodyLen {
		return fmt.Errorf("body length exceeds maximum %d", MaxBodyLen)
	}
	if kind.RequiresSubject() && subjectLen == 0 {
		return fmt.Errorf("subject is required for kind %d", kind)
	}
	return nil
}

func optionalUUID(input []byte) *UUID {
	var id UUID
	copy(id[:], input)
	for _, b := range id {
		if b != 0 {
			return &id
		}
	}
	return nil
}

func putU16(out []byte, value uint16) {
	out[0] = byte(value >> 8)
	out[1] = byte(value)
}

func putU32(out []byte, value uint32) {
	out[0] = byte(value >> 24)
	out[1] = byte(value >> 16)
	out[2] = byte(value >> 8)
	out[3] = byte(value)
}

func putU64(out []byte, value uint64) {
	out[0] = byte(value >> 56)
	out[1] = byte(value >> 48)
	out[2] = byte(value >> 40)
	out[3] = byte(value >> 32)
	out[4] = byte(value >> 24)
	out[5] = byte(value >> 16)
	out[6] = byte(value >> 8)
	out[7] = byte(value)
}

func readU16(input []byte) uint16 {
	return uint16(input[0])<<8 | uint16(input[1])
}

func readU32(input []byte) uint32 {
	return uint32(input[0])<<24 | uint32(input[1])<<16 | uint32(input[2])<<8 | uint32(input[3])
}

func readU64(input []byte) uint64 {
	return uint64(input[0])<<56 |
		uint64(input[1])<<48 |
		uint64(input[2])<<40 |
		uint64(input[3])<<32 |
		uint64(input[4])<<24 |
		uint64(input[5])<<16 |
		uint64(input[6])<<8 |
		uint64(input[7])
}
