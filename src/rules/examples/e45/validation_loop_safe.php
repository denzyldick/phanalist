<?php

final class Shape
{
    private const MAX = 64;

    public static function invalid(string $expression, string $path, string $message): \InvalidArgumentException
    {
        return new \InvalidArgumentException(sprintf('Invalid "%s" in {%s}: %s.', $path, $expression, $message));
    }

    public static function checkUtf8(string $value, string $expression, string $name): void
    {
        if (preg_match('//u', $value) === 1) {
            return;
        }

        throw self::invalid($expression, $name, 'variable values must be valid UTF-8');
    }

    public static function assertMap(array $value, string $path, string $expression, bool $allowNestedArrays, int $depth): void
    {
        if ($depth > self::MAX) {
            throw new \RuntimeException($path);
        }

        foreach ($value as $key => $member) {
            if ($member === null) {
                continue;
            }

            $memberPath = sprintf('%s[%s]', $path, (string) $key);

            if (is_string($key)) {
                self::checkUtf8($key, $expression, $memberPath);
            }

            if (is_scalar($member)) {
                if (is_string($member)) {
                    self::checkUtf8($member, $expression, $memberPath);
                }

                continue;
            }

            if (is_array($member) && $allowNestedArrays) {
                self::assertNested($memberPath, $member, $expression, $depth + 1);

                continue;
            }

            throw self::invalid($expression, $memberPath, 'bad');
        }
    }

    public static function assertNested(string $path, array $value, string $expression, int $depth): void
    {
        if ($depth > self::MAX) {
            throw new \RuntimeException($path);
        }

        foreach ($value as $key => $member) {
            if ($member === null) {
                continue;
            }

            $memberPath = sprintf('%s[%s]', $path, (string) $key);

            if (is_string($key)) {
                self::checkUtf8($key, $expression, $memberPath);
            }

            if (is_scalar($member)) {
                if (is_string($member)) {
                    self::checkUtf8($member, $expression, $memberPath);
                }

                continue;
            }

            if (is_array($member)) {
                self::assertNested($memberPath, $member, $expression, $depth + 1);

                continue;
            }

            throw self::invalid($expression, $memberPath, 'bad');
        }
    }
}
