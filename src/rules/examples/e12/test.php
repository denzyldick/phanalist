<?php

namespace App\Service;

class ValidService
{
    private $prop;
    private static $staticProp;

    public function __construct()
    {
        $this->prop = 1;
        self::$staticProp = 2;
    }

    public function valid()
    {
        $a = $this->prop;
        $b = self::$staticProp;
    }
}

class InvalidService
{
    private $prop;
    public static $staticProp;

    public function invalid()
    {
        $this->prop = 1; // Violation
        $this->prop++; // Violation
        self::$staticProp = 2; // Violation
    }

    public function nested()
    {
        if (true) {
            $this->prop = 3; // Violation
        }
    }
}

class ResettableService implements \Symfony\Contracts\Service\ResetInterface
{
    private $prop;

    public function reset()
    {
        $this->prop = null; // OK because of ResetInterface
    }
}
