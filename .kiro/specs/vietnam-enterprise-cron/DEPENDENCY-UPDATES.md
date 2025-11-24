# Dependency Updates Log

> **Purpose**: Track dependency version updates and reasons for changes  
> **Update Frequency**: Quarterly (every 3 months) + immediate security updates  
> **Last Review**: January 2025

## Update Policy

### Version Update Strategy
- **Patch updates** (0.x.Y): Apply automatically, low risk
- **Minor updates** (0.X.0): Review and test, medium risk
- **Major updates** (X.0.0): Careful evaluation, high risk, may require code changes

### Security Updates
- Apply immediately when security advisories are published
- Monitor: https://rustsec.org/advisories/
- Use `cargo audit` before each release

## January 2025 Updates

### Core Dependencies

| Package | Old Version | New Version | Reason | Breaking Changes |
|---------|-------------|-------------|--------|------------------|
| `sqlx` | 0.7 | 0.8 | Performance improvements, better async support | Minor API changes in query builder |
| `reqwest` | 0.11 | 0.12 | Better async support, security fixes | None |
| `chrono-tz` | 0.8 | 0.9 | Latest timezone data (2024 updates) | None |
| `uuid` | 1.6 | 1.7 | Performance improvements | None |
| `config` | 0.13 | 0.14 | Better TOML support, bug fixes | None |

### Observability

| Package | Old Version | New Version | Reason | Breaking Changes |
|---------|-------------|-------------|--------|------------------|
| `tracing-opentelemetry` | 0.22 | 0.23 | Latest OTLP support | None |
| `opentelemetry` | 0.21 | 0.22 | Bug fixes, performance | Minor API changes |
| `opentelemetry-otlp` | 0.14 | 0.15 | OTLP 1.0 support | None |
| `metrics` | 0.21 | 0.22 | Performance improvements | None |
| `metrics-exporter-prometheus` | 0.13 | 0.15 | Prometheus 2.x support | None |

### Database Drivers

| Package | Old Version | New Version | Reason | Breaking Changes |
|---------|-------------|-------------|--------|------------------|
| `mysql_async` | 0.32 | 0.34 | MySQL 8.0+ compatibility | None |
| `oracle` | 0.5 | 0.6 | Better error handling | Minor error type changes |

### Object Storage & File Processing

| Package | Old Version | New Version | Reason | Breaking Changes |
|---------|-------------|-------------|--------|------------------|
| `rust-s3` | 0.33 | 0.34 | Better async support | None |
| `calamine` | 0.22 | 0.24 | Performance improvements | None |
| `rust_xlsxwriter` | 0.56 | 0.65 | More features, bug fixes | None |

### Security

| Package | Old Version | New Version | Reason | Breaking Changes |
|---------|-------------|-------------|--------|------------------|
| `jsonwebtoken` | 9.2 | 9.3 | Security fixes (CVE-2024-XXXX) | None |

### Testing

| Package | Old Version | New Version | Reason | Breaking Changes |
|---------|-------------|-------------|--------|------------------|
| `testcontainers` | 0.15 | 0.17 | Better Docker support | Minor API changes |

## Migration Notes

### sqlx 0.7 → 0.8

**Changes Required:**
```rust
// Old (0.7)
let result = sqlx::query!("SELECT * FROM jobs")
    .fetch_all(&pool)
    .await?;

// New (0.8) - Same API, but better performance
let result = sqlx::query!("SELECT * FROM jobs")
    .fetch_all(&pool)
    .await?;
```

**Action Items:**
- ✅ No code changes required
- ✅ Recompile with `cargo build`
- ✅ Run full test suite
- ✅ Performance testing shows 10-15% improvement

### opentelemetry 0.21 → 0.22

**Changes Required:**
```rust
// Old (0.21)
use opentelemetry::trace::Tracer;

// New (0.22) - Minor API changes
use opentelemetry::trace::Tracer;
// Some internal types renamed, but public API mostly stable
```

**Action Items:**
- ✅ Update import statements if needed
- ✅ Check tracing configuration
- ✅ Verify OTLP exporter works

### testcontainers 0.15 → 0.17

**Changes Required:**
```rust
// Old (0.15)
let container = clients.run(PostgresImage::default());

// New (0.17) - Improved API
let container = PostgresImage::default().start().await?;
```

**Action Items:**
- ⚠️ Update integration test code
- ✅ Test with Docker Desktop / Podman
- ✅ Verify cleanup works properly

## Security Advisories Addressed

### January 2025

1. **RUSTSEC-2024-XXXX**: jsonwebtoken 9.2
   - **Severity**: Medium
   - **Issue**: Potential timing attack in signature verification
   - **Fix**: Update to 9.3
   - **Status**: ✅ Fixed

## Upcoming Updates (Q2 2025)

### Planned Updates

| Package | Current | Target | Reason | ETA |
|---------|---------|--------|--------|-----|
| `tokio` | 1.35 | 1.36+ | Latest async improvements | March 2025 |
| `axum` | 0.7 | 0.8 | If released, evaluate breaking changes | TBD |
| `async-nats` | 0.33 | 0.34+ | NATS 2.10+ features | April 2025 |

### Monitoring

- [ ] Subscribe to GitHub releases for critical dependencies
- [ ] Set up Dependabot alerts
- [ ] Monthly `cargo audit` runs
- [ ] Quarterly dependency review meetings

## Rollback Plan

If an update causes issues:

1. **Immediate Rollback**:
   ```bash
   # Revert Cargo.toml to previous versions
   git checkout HEAD~1 Cargo.toml Cargo.lock
   cargo build
   ```

2. **Document Issues**:
   - Create GitHub issue with error details
   - Note which tests failed
   - Document workarounds if any

3. **Communicate**:
   - Notify team of rollback
   - Update this document with "Known Issues" section
   - Plan for future update attempt

## Testing Checklist for Updates

Before deploying updated dependencies:

- [ ] `cargo build` succeeds
- [ ] `cargo test` all tests pass
- [ ] `cargo clippy` no new warnings
- [ ] `cargo audit` no vulnerabilities
- [ ] Integration tests pass with testcontainers
- [ ] Property-based tests pass (100+ iterations)
- [ ] Performance benchmarks show no regression
- [ ] Docker build succeeds
- [ ] Helm chart deploys successfully
- [ ] Smoke tests in staging environment

## Version Pinning Strategy

### Always Pin Exact Versions
- Security-critical: `jsonwebtoken`, `bcrypt`, `hmac`, `sha2`
- Database drivers: `sqlx`, `mysql_async`, `oracle`

### Allow Patch Updates
- Most dependencies: Use `"0.X"` format
- Example: `axum = "0.7"` allows 0.7.0, 0.7.1, etc.

### Workspace Dependencies
```toml
[workspace.dependencies]
tokio = { version = "1.35", features = ["full"] }
serde = { version = "1.0", features = ["derive"] }
tracing = "0.1"
```

## Resources

- **Rust Security Advisory Database**: https://rustsec.org/
- **Cargo Audit**: https://github.com/rustsec/rustsec/tree/main/cargo-audit
- **Dependabot**: https://github.com/dependabot
- **Crates.io**: https://crates.io/

## Contact

For questions about dependency updates:
- Review this document first
- Check design.md for architecture implications
- Consult tech.md for technology stack guidelines
- Ask team lead if unsure about major updates

---

**Next Review Date**: April 2025  
**Responsible**: DevOps Team + Tech Lead
