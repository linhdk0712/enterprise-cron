-- Seed default roles and admin user
-- Requirements: 19.1.1-14 - Default Admin and Regular User roles
-- Requirements: 19.1.80 - Default admin user (admin/admin123)

-- Insert Admin role with full permissions
-- Requirements: 19.1.2 - Admin role with 17 permissions
INSERT INTO roles (id, name, permissions, created_at)
VALUES (
    '00000000-0000-0000-0000-000000000001'::uuid,
    'Admin',
    '["job:read", "job:write", "job:execute", "job:delete", "job:import", "job:export", "execution:read", "execution:stop", "variable:read", "variable:write", "variable:encrypt", "webhook:read", "webhook:write", "user:manage", "role:assign", "system:config", "system:audit", "dashboard:admin"]'::jsonb,
    NOW()
) ON CONFLICT (name) DO NOTHING;

-- Insert Regular User role with limited permissions
-- Requirements: 19.1.3 - Regular User role with 5 permissions
INSERT INTO roles (id, name, permissions, created_at)
VALUES (
    '00000000-0000-0000-0000-000000000002'::uuid,
    'Regular User',
    '["job:read", "job:execute", "execution:read", "variable:read", "dashboard:user"]'::jsonb,
    NOW()
) ON CONFLICT (name) DO NOTHING;

-- Create default admin user
-- Requirements: 19.1.80 - Default admin user with username "admin" and password "admin123"
-- Password hash for "admin123" using bcrypt (cost 12)
-- Generated with: bcrypt.hashpw(b"admin123", bcrypt.gensalt())
INSERT INTO users (id, username, password_hash, email, enabled, created_at, updated_at)
VALUES (
    '00000000-0000-0000-0000-000000000001'::uuid,
    'admin',
    '$2b$12$BZFHZuZf2AHVzag.lr00fO7YTtKzDjo9Hd3S8SBVX2rXXCBrV.W9u', -- admin123
    'admin@example.com',
    true,
    NOW(),
    NOW()
) ON CONFLICT (username) DO NOTHING;

-- Assign Admin role to default admin user
INSERT INTO user_roles (user_id, role_id, created_at)
VALUES (
    '00000000-0000-0000-0000-000000000001'::uuid,
    '00000000-0000-0000-0000-000000000001'::uuid,
    NOW()
) ON CONFLICT (user_id, role_id) DO NOTHING;
