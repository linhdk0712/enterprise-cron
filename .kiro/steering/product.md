# Product Overview

## Vietnam Enterprise Cron System

A production-ready, distributed job scheduling and execution platform built in Rust to replace Java Quartz + Spring Batch implementations in Vietnamese enterprises (banking, telco, e-commerce).

### Core Capabilities

- **Distributed Job Scheduling**: Cron expressions (Quartz syntax with second precision), fixed delay, fixed rate, and one-time jobs with timezone support (default: Asia/Ho_Chi_Minh)
- **Multiple Job Types**: HTTP requests (GET/POST/PUT with Basic/Bearer/OAuth2 auth) and database queries (PostgreSQL, MySQL, Oracle 19c)
- **Exactly-Once Execution**: Redis RedLock for distributed locking, idempotency keys, and NATS JetStream for reliable message delivery
- **Variable Management**: Global and job-specific variables with encryption for sensitive data, template substitution in URLs, headers, bodies, and SQL queries
- **Enterprise Reliability**: Exponential backoff retry (up to 10 attempts), circuit breaker pattern, dead letter queue, and configurable timeouts
- **Real-Time Dashboard**: HTMX-based responsive UI with Server-Sent Events for live updates
- **Flexible Authentication**: Keycloak integration or database-based authentication with RBAC (role-based access control)
- **Comprehensive Observability**: Structured logging (JSON), Prometheus metrics, OpenTelemetry tracing, and automated alerting

### Target Users

- System administrators managing scheduled business processes
- Platform engineers ensuring high availability and scalability
- DevOps engineers monitoring system health
- Security administrators controlling access and auditing operations
