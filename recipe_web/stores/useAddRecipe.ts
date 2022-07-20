import create from 'zustand';
import { combine } from 'zustand/middleware';
import { AddIngredient, DifficultyLevel, MealType } from '../utils/types';

export const useAddIngredient = create(
  combine(
    {
      name: '',
      description: '',
      prep_time: 0,
      cook_time: 0,
      difficulty: 'easy',
      steps: [] as string[],
      cuisine: 'hungarian',
      meal_type: 'breakfast' as MealType,
      ingredients: [] as AddIngredient[],
    },
    (set) => ({
      setName: (value: string) => set(() => ({ name: value })),
      setDescription: (value: string) => set(() => ({ description: value })),
      setPrepTime: (value: number) => set(() => ({ prep_time: value })),
      setCookTime: (value: number) => set(() => ({ cook_time: value })),
      setDifficulty: (value: DifficultyLevel) => set(() => ({ difficulty: value })),
      setSteps: (value: string[]) => set(() => ({ steps: value })),
      pushStep: (value: string) =>
        set(({ steps, ...rest }) => ({ ...rest, steps: [...steps, value] })),
      removeStep: (value: string) =>
        set(({ steps, ...rest }) => ({
          ...rest,
          steps: steps.filter((step: any) => step !== value),
        })),
      setCuisine: (value: string) => set(() => ({ cuisine: value })),
      setMealType: (value: MealType) => set(() => ({ meal_type: value })),
      setIngredients: (value: AddIngredient[]) => set(() => ({ ingredients: value })),
      pushIngredient: (value: AddIngredient) =>
        set(({ ingredients, ...rest }) => ({ ...rest, ingredients: [...ingredients, value] })),
      removeIngredient: (value: string) =>
        set(({ ingredients, ...rest }) => ({
          ...rest,
          ingredients: ingredients.filter((i: AddIngredient) => i.name !== value),
        })),
    })
  )
);