# Task 19.5 Implementation: Authentication Endpoints

## Overview
Implemented three authentication endpoints for database mode authentication as specified in requirements 10.2 and 10.3.

## Endpoints Implemented

### 1. POST /api/auth/login
**Purpose**: Authenticate users with username and password (database mode)

**Requirements Satisfied**:
- 10.2: Validate credentials against bcrypt-hashed passwords in System Database
- 10.3: Generate JWT token on successful login

**Request Body**:
```json
{
  "username": "string",
  "password": "string"
}
```

**Response** (200 OK):
```json
{
  "data": {
    "token": "jwt-token-string",
    "expires_at": 1234567890
  }
}
```

**Error Responses**:
- 400: Validation error (empty username/password)
- 401: Invalid credentials or disabled account
- 500: Internal server error

**Implementation Details**:
- Validates input (non-empty username and password)
- Creates JwtService from configuration
- Uses DatabaseAuthService to authenticate user
- Verifies password using bcrypt
- Checks if user account is enabled
- Retrieves user permissions from database
- Generates JWT token with user claims
- Returns token with expiration timestamp

### 2. POST /api/auth/refresh
**Purpose**: Refresh an existing JWT token to extend session

**Requirements Satisfied**:
- 10.3: Generate new JWT token from valid existing token

**Request Body**:
```json
{
  "token": "existing-jwt-token"
}
```

**Response** (200 OK):
```json
{
  "data": {
    "token": "new-jwt-token-string",
    "expires_at": 1234567890
  }
}
```

**Error Responses**:
- 400: Validation error (empty token)
- 401: Invalid or expired token
- 500: Internal server error

**Implementation Details**:
- Validates input (non-empty token)
- Creates JwtService from configuration
- Validates existing token
- Extracts claims from existing token
- Generates new token with same claims
- Returns new token with new expiration timestamp

### 3. POST /api/users
**Purpose**: Create a new user account (database mode)

**Requirements Satisfied**:
- 10.2: Create user with bcrypt-hashed password
- 10.13: Store user with role assignments in System Database

**Request Body**:
```json
{
  "username": "string",
  "password": "string",
  "email": "string (optional)",
  "role_ids": ["uuid", "uuid", ...]
}
```

**Response** (200 OK):
```json
{
  "data": {
    "id": "uuid",
    "username": "string",
    "email": "string or null",
    "enabled": true,
    "created_at": "2025-01-01T00:00:00Z"
  }
}
```

**Error Responses**:
- 400: Validation error (empty username, empty password, password < 8 chars)
- 409: Username already exists
- 500: Internal server error

**Implementation Details**:
- Validates input (non-empty username, password >= 8 characters)
- Creates JwtService and DatabaseAuthService
- Hashes password using bcrypt (DEFAULT_COST)
- Creates user in database
- Assigns specified roles to user
- Returns user information (without password hash)

## Security Features

1. **Password Hashing**: All passwords are hashed using bcrypt with DEFAULT_COST (currently 12)
2. **JWT Tokens**: Tokens are signed with HS256 algorithm using configured secret
3. **Token Expiration**: Tokens expire after configured hours (default 24)
4. **Input Validation**: All inputs are validated before processing
5. **Error Handling**: Sensitive information is not leaked in error messages
6. **Audit Logging**: All authentication events are logged with structured logging

## Configuration

The endpoints use the following configuration from `Settings`:

```toml
[auth]
mode = "database"  # or "keycloak"
jwt_secret = "your-secret-key"
jwt_expiration_hours = 24
```

## Testing

Unit tests are included in the module to verify:
- Request/response serialization/deserialization
- Data structure conversions
- Error handling

Integration tests require a running PostgreSQL database and are not included in this implementation.

## Routes Configuration

The endpoints are configured in `api/src/routes.rs`:
- `/api/auth/login` - Public route (no authentication)
- `/api/auth/refresh` - Public route (no authentication)
- `/api/users` - Protected route (requires authentication)

## Dependencies

The implementation uses:
- `common::auth::JwtService` - JWT token encoding/decoding
- `common::auth::DatabaseAuthService` - User authentication logic
- `common::db::repositories::user::UserRepository` - Database operations
- `bcrypt` - Password hashing
- `jsonwebtoken` - JWT token handling
- `chrono` - Timestamp handling

## Error Handling

All errors are properly handled and logged:
- Authentication failures are logged with username (not password)
- Database errors are logged with context
- Configuration errors are logged immediately
- All errors return appropriate HTTP status codes

## Compliance

This implementation follows:
- RECC 2025 rules (no unwrap/expect, proper error handling)
- Requirements 10.2, 10.3, 10.13
- Design document specifications
- Existing code patterns and conventions
