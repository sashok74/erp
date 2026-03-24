-- V001: Создание schemas для modular monolith.
-- Каждый BC получит свою schema.
CREATE SCHEMA IF NOT EXISTS common;
CREATE SCHEMA IF NOT EXISTS warehouse;
-- Будущие: CREATE SCHEMA IF NOT EXISTS finance;
