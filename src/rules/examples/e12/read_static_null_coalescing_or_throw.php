<?php

namespace App\Service\e12;

class ReadArrayNullCoalescingOrThrow
{
    private static ?string $guid = null;
    
    public function __construct()
    {
    }

    public static function getGuidNotNull(): string
    {
        return self::$guid ?? throw new \RuntimeException('Missing static guid');
    }
}
