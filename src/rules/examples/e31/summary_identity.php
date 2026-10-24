<?php

function identity_value($value)
{
    return $value;
}

class IdentityMethod
{
    public static function pass($value)
    {
        return $value;
    }

    public function instancePass($value)
    {
        return $value;
    }

    public function useInstance(): float
    {
        return 1 / $this->instancePass(0);
    }
}

function use_static_identity(): float
{
    return 1 / IdentityMethod::pass(0);
}

function use_function_identity(): float
{
    return 1 / identity_value(0);
}

function safe_identity(): float
{
    return 1 / identity_value(2);
}
