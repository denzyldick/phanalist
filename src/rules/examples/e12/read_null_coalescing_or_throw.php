<?php

namespace App\Service\e12;

class ReadArrayNullCoalescingOrThrow
{
    public function __construct(private ?string $guid = null)
    {
    }

    public function getGuidNotNull(): string
    {
        return $this->guid ?? throw new \RuntimeException('Missing guid');
    }
}
