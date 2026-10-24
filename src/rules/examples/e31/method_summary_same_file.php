<?php

class MethodSummary
{
    public static function staticDivisor(): int
    {
        return 0;
    }

    private function instanceDivisor(): int
    {
        return 0;
    }

    public static function useStatic(): float
    {
        return 1 / self::staticDivisor();
    }

    public function useInstance(): float
    {
        return 1 / $this->instanceDivisor();
    }
}
