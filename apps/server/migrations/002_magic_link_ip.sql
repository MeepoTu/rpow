ALTER TABLE magic_links ADD COLUMN IF NOT EXISTS ip_addr INET;
CREATE INDEX IF NOT EXISTS magic_links_ip_idx ON magic_links(ip_addr, created_at);
CREATE INDEX IF NOT EXISTS magic_links_email_created_idx ON magic_links(email, created_at);
